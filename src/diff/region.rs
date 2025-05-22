use super::{Diff, blob::BlobDiff, myers::MyersDiff};
use crate::{
    mca::{ChunkWithTimestamp, CompressionType, LazyChunk, MCABuilder, MCAReader},
    object::{Serde, SerdeError},
    util::create_chunk_ixz_iter,
};
use fastnbt::Value;
use std::collections::BTreeMap;

#[derive(Debug)]
struct NbtDiff {
    block_entities: MyersDiff,
    sections: Vec<MyersDiff>,
    others: MyersDiff,
}
fn wrap_with_root_compound(value: Value) -> Value {
    Value::Compound(BTreeMap::from([("root".to_string(), value)]))
}
fn unwrap_with_root_compound(value: Value) -> Value {
    match value {
        Value::Compound(mut map) => map.remove("root").unwrap(),
        _ => panic!("root compound not exists"),
    }
}
fn pop_block_entities_from(compound_map: &mut BTreeMap<String, Value>) -> Vec<u8> {
    let block_entities = compound_map.remove("block_entities").unwrap();
    let wrapped = wrap_with_root_compound(block_entities);
    fastnbt::to_bytes(&wrapped).unwrap()
}
impl Diff for NbtDiff {
    fn from_compare(old_nbt: &[u8], new_nbt: &[u8]) -> Self
    where
        Self: Sized,
    {
        let old: Value = fastnbt::from_bytes(old_nbt).unwrap();
        let mut old = match old {
            Value::Compound(x) => x,
            _ => panic!("invalid old nbt"),
        };
        let new: Value = fastnbt::from_bytes(new_nbt).unwrap();
        let mut new = match new {
            Value::Compound(x) => x,
            _ => panic!("invalid new nbt"),
        };

        let diff_block_entities;
        {
            let old_block_entities = pop_block_entities_from(&mut old);
            let new_block_entities = pop_block_entities_from(&mut new);
            diff_block_entities = MyersDiff::from_compare(&old_block_entities, &new_block_entities);
        }

        let diff_sections;
        {
            let old_sections = old.remove("sections").unwrap();
            let old_sections = match old_sections {
                Value::List(x) => x,
                _ => panic!("invalid old nbt"),
            };
            let new_sections = new.remove("sections").unwrap();
            let new_sections = match new_sections {
                Value::List(x) => x,
                _ => panic!("invalid new nbt"),
            };
            assert_eq!(old_sections.len(), new_sections.len());

            let mut mut_diff_sections = Vec::with_capacity(old_sections.len());
            for (old, new) in old_sections.iter().zip(new_sections.iter()) {
                let old = fastnbt::to_bytes(old).unwrap();
                let new = fastnbt::to_bytes(new).unwrap();
                let diff = MyersDiff::from_compare(&old, &new);
                mut_diff_sections.push(diff);
            }
            diff_sections = mut_diff_sections;
        }

        let diff_others;
        {
            let old_others = fastnbt::to_bytes(&Value::Compound(old)).unwrap();
            let new_others = fastnbt::to_bytes(&Value::Compound(new)).unwrap();
            diff_others = MyersDiff::from_compare(&old_others, &new_others);
        }

        Self {
            block_entities: diff_block_entities,
            sections: diff_sections,
            others: diff_others,
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        let block_entities =
            MyersDiff::from_squash(&base.block_entities, &squashing.block_entities);
        let sections = base
            .sections
            .iter()
            .zip(squashing.sections.iter())
            .map(|(base, squashing)| MyersDiff::from_squash(base, squashing))
            .collect();
        let others = MyersDiff::from_squash(&base.others, &squashing.others);
        Self {
            block_entities,
            sections,
            others,
        }
    }

    fn patch(&self, old_nbt: &[u8]) -> Vec<u8> {
        let old: Value = fastnbt::from_bytes(old_nbt).unwrap();
        let mut old = match old {
            Value::Compound(x) => x,
            _ => panic!("invalid old nbt"),
        };

        let block_entities;
        {
            let old_block_entities = pop_block_entities_from(&mut old);
            let new_block_entities = self.block_entities.patch(&old_block_entities);
            let new_block_entities = fastnbt::from_bytes(&new_block_entities).unwrap();
            block_entities = unwrap_with_root_compound(new_block_entities);
        }

        let sections: Vec<Value>;
        {
            let old_sections = old.remove("sections").unwrap();
            let old_sections = match old_sections {
                Value::List(x) => x,
                _ => panic!("invalid old nbt"),
            };
            sections = old_sections
                .iter()
                .zip(self.sections.iter())
                .map(|(old, diff)| {
                    let old = fastnbt::to_bytes(old).unwrap();
                    let new = diff.patch(&old);
                    let new = fastnbt::from_bytes(&new).unwrap();
                    new
                })
                .collect()
        }

        let mut others;
        {
            let old_others = fastnbt::to_bytes(&Value::Compound(old)).unwrap();
            let new_others = self.others.patch(&old_others);
            let wrapped_others: Value = fastnbt::from_bytes(&new_others).unwrap();
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("invalid new nbt"),
            }
        }

        others.insert("sections".to_string(), Value::List(sections));
        others.insert("block_entities".to_string(), block_entities);

        fastnbt::to_bytes(&Value::Compound(others)).unwrap()
    }

    fn revert(&self, new_nbt: &[u8]) -> Vec<u8> {
        let new: Value = fastnbt::from_bytes(new_nbt).unwrap();
        let mut new = match new {
            Value::Compound(x) => x,
            _ => panic!("invalid new nbt"),
        };

        let block_entities;
        {
            let new_block_entities = pop_block_entities_from(&mut new);
            let old_block_entities = self.block_entities.revert(&new_block_entities);
            let old_block_entities = fastnbt::from_bytes(&old_block_entities).unwrap();
            block_entities = unwrap_with_root_compound(old_block_entities);
        }

        let sections: Vec<Value>;
        {
            let new_sections = new.remove("sections").unwrap();
            let new_sections = match new_sections {
                Value::List(x) => x,
                _ => panic!("invalid new nbt"),
            };
            sections = new_sections
                .iter()
                .zip(self.sections.iter())
                .map(|(new_section, diff)| {
                    let new_bytes = fastnbt::to_bytes(new_section).unwrap();
                    let old_bytes = diff.revert(&new_bytes);
                    fastnbt::from_bytes(&old_bytes).unwrap()
                })
                .collect();
        }

        let mut others;
        {
            let new_others = fastnbt::to_bytes(&Value::Compound(new)).unwrap();
            let old_others = self.others.revert(&new_others);
            let wrapped_others: Value = fastnbt::from_bytes(&old_others).unwrap();
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("invalid old nbt"),
            };
        }

        others.insert("sections".to_string(), Value::List(sections));
        others.insert("block_entities".to_string(), block_entities);

        fastnbt::to_bytes(&Value::Compound(others)).unwrap()
    }
}
impl Serde for NbtDiff {
    fn serialize(&self) -> Result<Vec<u8>, SerdeError> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, SerdeError>
    where
        Self: Sized,
    {
        todo!()
    }
}
#[derive(Debug)]
enum ChunkWithTimestampDiff {
    NotExists,
    Minor(i32, NbtDiff),
    Major(u32, BlobDiff),
}
#[derive(Debug)]
pub struct RegionDiff {
    chunks: [ChunkWithTimestampDiff; 1024],
}

// suitable for region/*.mca
impl Diff for RegionDiff {
    fn from_compare(old: &[u8], new: &[u8]) -> Self
    where
        Self: Sized,
    {
        let reader_old = MCAReader::from_bytes(old).unwrap();
        let reader_new = MCAReader::from_bytes(new).unwrap();
        let mut chunks = [const { ChunkWithTimestampDiff::NotExists }; 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            let old = reader_old.get_chunk_lazily(x, z);
            let new = reader_new.get_chunk_lazily(x, z);
            let chunk = match (old, new) {
                (LazyChunk::Unloaded, _) => panic!("old chunk is unloaded"),
                (_, LazyChunk::Unloaded) => panic!("new chunk is unloaded"),
                (LazyChunk::NotExists, LazyChunk::NotExists) => ChunkWithTimestampDiff::NotExists,
                (LazyChunk::NotExists, LazyChunk::Some(chunk)) => ChunkWithTimestampDiff::Major(
                    chunk.timestamp,
                    BlobDiff::from_compare(&[], &chunk.nbt),
                ),
                (LazyChunk::Some(chunk), LazyChunk::NotExists) => {
                    ChunkWithTimestampDiff::Major(0, BlobDiff::from_compare(&chunk.nbt, &[]))
                }
                (LazyChunk::Some(chunk_old), LazyChunk::Some(chunk_new)) => {
                    ChunkWithTimestampDiff::Minor(
                        chunk_new.timestamp as i32 - chunk_old.timestamp as i32,
                        NbtDiff::from_compare(&chunk_old.nbt, &chunk_new.nbt),
                    )
                }
            };
            chunks[i] = chunk;
        }
        Self { chunks }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        todo!()
    }

    fn patch(&self, old: &[u8]) -> Vec<u8> {
        let reader = MCAReader::from_bytes(old).unwrap();
        let mut builder = MCABuilder::new();
        let mut chunks_holder = Vec::with_capacity(1024);
        for (i, x, z) in create_chunk_ixz_iter() {
            let lazy_chunk = reader.get_chunk_lazily(x, z);
            let chunk_diff = &self.chunks[i];
            let new_chunk = match (lazy_chunk, chunk_diff) {
                // LazyChunk::Unloaded
                (LazyChunk::Unloaded, _) => panic!("old chunk is unloaded"),

                // LazyChunk::NotExists
                (LazyChunk::NotExists, ChunkWithTimestampDiff::NotExists) => None,
                (
                    LazyChunk::NotExists,
                    ChunkWithTimestampDiff::Major(timestamp_diff, chunk_diff),
                ) => Some(ChunkWithTimestamp {
                    timestamp: *timestamp_diff,
                    nbt: chunk_diff.patch(&[]),
                }),
                (LazyChunk::NotExists, ChunkWithTimestampDiff::Minor(_, _)) => panic!(
                    "old chunk not exists, but has minor diff, expected also not exists or major diff"
                ),

                // LazyChunk::Some
                (LazyChunk::Some(_), ChunkWithTimestampDiff::NotExists) => {
                    panic!("old chunk exists, but diff report not exists, expected has minor diff")
                }
                (LazyChunk::Some(_), ChunkWithTimestampDiff::Major(_, _)) => {
                    panic!("old chunk exists, but has major diff, expected has minor diff")
                }
                (
                    LazyChunk::Some(old_chunk),
                    ChunkWithTimestampDiff::Minor(timestamp_diff, chunk_diff),
                ) => Some(ChunkWithTimestamp {
                    timestamp: old_chunk
                        .timestamp
                        .checked_add_signed(*timestamp_diff)
                        .expect("timestamp overflowed"),
                    nbt: chunk_diff.patch(&old_chunk.nbt),
                }),
            };
            if let Some(chunk) = new_chunk {
                chunks_holder.push((x, z, chunk));
            }
        }
        for (x, z, chunk) in chunks_holder.iter() {
            builder.set_chunk(*x, *z, &chunk);
        }
        builder.to_bytes(CompressionType::Zlib)
    }

    fn revert(&self, new: &[u8]) -> Vec<u8> {
        let reader = MCAReader::from_bytes(new).unwrap();
        let mut builder = MCABuilder::new();
        let mut chunks_holder = Vec::with_capacity(1024);
        for (i, x, z) in create_chunk_ixz_iter() {
            let lazy_chunk = reader.get_chunk_lazily(x, z);
            let chunk_diff = &self.chunks[i];
            let old_chunk = match (lazy_chunk, chunk_diff) {
                // LazyChunk::Unloaded
                (LazyChunk::Unloaded, _) => panic!("new chunk is unloaded"),

                // LazyChunk::NotExists
                (LazyChunk::NotExists, ChunkWithTimestampDiff::NotExists) => None,
                (
                    LazyChunk::NotExists,
                    ChunkWithTimestampDiff::Major(timestamp_diff, chunk_diff),
                ) => Some(ChunkWithTimestamp {
                    timestamp: *timestamp_diff,
                    nbt: chunk_diff.revert(&[]),
                }),
                (LazyChunk::NotExists, ChunkWithTimestampDiff::Minor(_, _)) => panic!(
                    "new chunk not exists, but has minor diff, expected also not exists or major diff"
                ),

                // LazyChunk::Some
                (LazyChunk::Some(_), ChunkWithTimestampDiff::NotExists) => {
                    panic!("new chunk exists, but diff report not exists, expected has minor diff")
                }
                (LazyChunk::Some(_), ChunkWithTimestampDiff::Major(_, _)) => {
                    panic!("new chunk exists, but has major diff, expected has minor diff")
                }
                (
                    LazyChunk::Some(new_chunk),
                    ChunkWithTimestampDiff::Minor(timestamp_diff, chunk_diff),
                ) => Some(ChunkWithTimestamp {
                    timestamp: new_chunk
                        .timestamp
                        .checked_add_signed(-*timestamp_diff)
                        .expect("timestamp overflowed"),
                    nbt: chunk_diff.revert(&new_chunk.nbt),
                }),
            };
            if let Some(chunk) = old_chunk {
                chunks_holder.push((x, z, chunk));
            }
        }
        for (x, z, chunk) in chunks_holder.iter() {
            builder.set_chunk(*x, *z, &chunk);
        }
        builder.to_bytes(CompressionType::Zlib)
    }
}
impl Serde for RegionDiff {
    fn serialize(&self) -> Result<Vec<u8>, SerdeError> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, SerdeError>
    where
        Self: Sized,
    {
        todo!()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mca::{LazyChunk, MCAReader},
        util::rearranged_nbt,
    };
    use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};

    fn get_test_chunk(path: &str, seed: u64) -> impl Iterator<Item = Vec<u8>> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        let mut xzs = [(0, 0); 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            xzs[i] = (x, z);
        }
        let mut rng = StdRng::seed_from_u64(seed);
        xzs.shuffle(&mut rng);
        xzs.into_iter()
            .map(move |(x, z)| reader.get_chunk(x, z).unwrap().unwrap().nbt.clone())
    }
    #[test]
    fn test_mca_timestamp_nbt() {
        let reader_old = MCAReader::from_file("./resources/mca/r.1.2.20250511.mca", false).unwrap();
        let reader_new = MCAReader::from_file("./resources/mca/r.1.2.20250512.mca", false).unwrap();
        for (_, x, z) in create_chunk_ixz_iter() {
            let (timestamp_old, nbt_old) = match reader_old.get_chunk_lazily(x, z) {
                LazyChunk::Some(chunk) => (chunk.timestamp, rearranged_nbt(&chunk.nbt).unwrap()),
                _ => panic!("chunk should loaded"),
            };
            let (timestamp_new, nbt_new) = match reader_new.get_chunk_lazily(x, z) {
                LazyChunk::Some(chunk) => (chunk.timestamp, rearranged_nbt(&chunk.nbt).unwrap()),
                _ => panic!("chunk should loaded"),
            };
            if timestamp_old == timestamp_new {
                assert_eq!(nbt_old, nbt_new);
            } else {
                assert_ne!(nbt_old, nbt_new);
            }
        }
    }
    mod test_nbt_diff {
        use super::*;
        mod test_in_continuous_data {
            use super::*;
            #[test]
            fn test_diff_patch_revert() -> () {
                let mut old_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", 114514);
                let mut new_iter = get_test_chunk("./resources/mca/r.1.2.20250516.mca", 114514);
                for _ in 0..100 {
                    let old = old_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                    let new = new_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                    let diff = NbtDiff::from_compare(&old, &new);
                    let patched_old = diff.patch(&old);
                    let reverted_new = diff.revert(&new);

                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&new),
                        fastnbt::from_bytes::<Value>(&patched_old)
                    );
                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&old),
                        fastnbt::from_bytes::<Value>(&reverted_new)
                    );
                }
            }
            #[test]
            fn test_diff_squash() -> () {
                let mut v0_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", 114514);
                let mut v1_iter = get_test_chunk("./resources/mca/r.1.2.20250513.mca", 114514);
                let mut v2_iter = get_test_chunk("./resources/mca/r.1.2.20250515.mca", 114514);
                for _ in 0..100 {
                    let v0 = v0_iter.next().unwrap();
                    let v1 = v1_iter.next().unwrap();
                    let v2 = v2_iter.next().unwrap();
                    let diff_v01 = NbtDiff::from_compare(&v0, &v1);
                    let diff_v12 = NbtDiff::from_compare(&v1, &v2);
                    let merged_diff = NbtDiff::from_squash(&diff_v01, &diff_v12);
                    let patched_v0 = merged_diff.patch(&v0);
                    let reverted_v2 = merged_diff.revert(&v2);

                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&v2),
                        fastnbt::from_bytes::<Value>(&patched_v0)
                    );
                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&v0),
                        fastnbt::from_bytes::<Value>(&reverted_v2)
                    );
                }
            }
        }
        mod test_in_noncontinuous_data {
            use super::*;
            #[test]
            fn test_diff_patch_revert() -> () {
                let mut old_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", 114514);
                let mut new_iter = get_test_chunk("./resources/mca/r.1.2.20250516.mca", 1919810);
                for _ in 0..3 {
                    let old = old_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                    let new = new_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                    let diff = NbtDiff::from_compare(&old, &new);
                    let patched_old = diff.patch(&old);
                    let reverted_new = diff.revert(&new);

                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&new),
                        fastnbt::from_bytes::<Value>(&patched_old)
                    );
                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&old),
                        fastnbt::from_bytes::<Value>(&reverted_new)
                    );
                }
            }
            #[test]
            fn test_diff_squash() -> () {
                let mut v0_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", 114514);
                let mut v1_iter = get_test_chunk("./resources/mca/r.1.2.20250513.mca", 1919810);
                let mut v2_iter = get_test_chunk("./resources/mca/r.1.2.20250515.mca", 19260817);
                for _ in 0..3 {
                    let v0 = v0_iter.next().unwrap();
                    let v1 = v1_iter.next().unwrap();
                    let v2 = v2_iter.next().unwrap();
                    let diff_v01 = NbtDiff::from_compare(&v0, &v1);
                    let diff_v12 = NbtDiff::from_compare(&v1, &v2);
                    let merged_diff = NbtDiff::from_squash(&diff_v01, &diff_v12);
                    let patched_v0 = merged_diff.patch(&v0);
                    let reverted_v2 = merged_diff.revert(&v2);

                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&v2),
                        fastnbt::from_bytes::<Value>(&patched_v0)
                    );
                    assert_eq!(
                        fastnbt::from_bytes::<Value>(&v0),
                        fastnbt::from_bytes::<Value>(&reverted_v2)
                    );
                }
            }
        }
    }
    mod test_region_diff {
        use super::*;
        fn build_test_mca(path: &str, chunks: usize, seed: u64) -> Vec<u8> {
            use rand::prelude::*;
            let mut rng = StdRng::seed_from_u64(seed);
            let avaliable_indexes: Vec<_> = create_chunk_ixz_iter().collect();
            let reader = MCAReader::from_file(path, false).unwrap();
            let mut builder = MCABuilder::new();
            for (_, x, z) in avaliable_indexes
                .into_iter()
                .choose_multiple(&mut rng, chunks)
            {
                let chunk = reader.get_chunk_lazily(x, z);
                if let LazyChunk::Some(chunk) = chunk {
                    builder.set_chunk(x, z, chunk);
                } else {
                    panic!("chunk is not avaliable");
                }
            }
            builder.to_bytes(CompressionType::Zlib)
        }
        fn assert_mca_eq(a: &[u8], b: &[u8]) {
            let mut reader_a = MCAReader::from_bytes(a).unwrap();
            let mut reader_b = MCAReader::from_bytes(b).unwrap();
            for (_, x, z) in create_chunk_ixz_iter() {
                let chunk_a = reader_a.get_chunk(x, z).unwrap();
                let chunk_b = reader_b.get_chunk(x, z).unwrap();
                assert_eq!(chunk_a, chunk_b);
            }
        }
        #[test]
        fn test_diff_patch_revert() -> () {
            let paths = [
                "./resources/mca/r.1.2.20250511.mca",
                "./resources/mca/r.1.2.20250512.mca",
                "./resources/mca/r.1.2.20250513.mca",
                "./resources/mca/r.1.2.20250514.mca",
                "./resources/mca/r.1.2.20250515.mca",
                "./resources/mca/r.1.2.20250516.mca",
            ];
            let seed = 114514;
            for path_old_new in paths.windows(2) {
                let old = build_test_mca(path_old_new[0], 100, seed);
                let new = build_test_mca(path_old_new[1], 100, seed);
                let diff = RegionDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);

                assert_mca_eq(&new, &patched_old);
                assert_mca_eq(&old, &reverted_new);
            }
        }
    }
}
