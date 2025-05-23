use crate::{
    diff::{
        Diff,
        base::{BlobDiff, MyersDiff},
        nbt::ChunkDiff,
    },
    mca::{ChunkWithTimestamp, CompressionType, LazyChunk, MCABuilder, MCAReader},
    object::{Serde, SerdeError},
    util::create_chunk_ixz_iter,
};

#[derive(Debug, Clone)]
enum ChunkWithTimestampDiff {
    BothNotExist,
    Create(i32, BlobDiff),
    Delete(i32, BlobDiff),
    Update(i32, ChunkDiff),
    NoChange,
}
impl ChunkWithTimestampDiff {
    pub fn get_description(&self) -> String {
        match self {
            ChunkWithTimestampDiff::BothNotExist => {
                "report both old chunk and new chunk not exist".to_string()
            }
            ChunkWithTimestampDiff::Create(_, _) => "is a create diff".to_string(),
            ChunkWithTimestampDiff::Delete(_, _) => "is a delete diff".to_string(),
            ChunkWithTimestampDiff::Update(_, _) => "is a update diff".to_string(),
            ChunkWithTimestampDiff::NoChange => {
                "report there's no change between old chunk and new chunk".to_string()
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct McaDiff {
    chunks: [ChunkWithTimestampDiff; 1024],
}

impl Diff for McaDiff {
    fn from_compare(old: &[u8], new: &[u8]) -> Self
    where
        Self: Sized,
    {
        let reader_old = MCAReader::from_bytes(old).unwrap();
        let reader_new = MCAReader::from_bytes(new).unwrap();
        let mut chunks = [const { ChunkWithTimestampDiff::BothNotExist }; 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            let old_ts = reader_old.get_timestamp(x, z);
            let new_ts = reader_new.get_timestamp(x, z);
            let ts_diff = new_ts as i32 - old_ts as i32;
            let chunk = match (old_ts, new_ts, ts_diff) {
                (0, 0, _) => ChunkWithTimestampDiff::BothNotExist,
                (_, _, 0) => ChunkWithTimestampDiff::NoChange,
                _ => {
                    let old = reader_old.get_chunk_lazily(x, z);
                    let new = reader_new.get_chunk_lazily(x, z);
                    match (old, new) {
                        (LazyChunk::Unloaded, _) => panic!("old chunk is unloaded"),
                        (_, LazyChunk::Unloaded) => panic!("new chunk is unloaded"),
                        (LazyChunk::NotExists, LazyChunk::NotExists) => {
                            ChunkWithTimestampDiff::BothNotExist
                        }
                        (LazyChunk::NotExists, LazyChunk::Some(chunk)) => {
                            ChunkWithTimestampDiff::Create(
                                chunk.timestamp as i32 - 0,
                                BlobDiff::from_compare(&[], &chunk.nbt),
                            )
                        }
                        (LazyChunk::Some(chunk), LazyChunk::NotExists) => {
                            ChunkWithTimestampDiff::Delete(
                                0 - chunk.timestamp as i32,
                                BlobDiff::from_compare(&chunk.nbt, &[]),
                            )
                        }
                        (LazyChunk::Some(chunk_old), LazyChunk::Some(chunk_new)) => {
                            let ts_diff = chunk_new.timestamp as i32 - chunk_old.timestamp as i32;
                            if ts_diff == 0 {
                                ChunkWithTimestampDiff::NoChange
                            } else {
                                ChunkWithTimestampDiff::Update(
                                    ts_diff,
                                    ChunkDiff::from_compare(&chunk_old.nbt, &chunk_new.nbt),
                                )
                            }
                        }
                    }
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
        let mut squashed_chunks = [const { ChunkWithTimestampDiff::BothNotExist }; 1024];
        for (i, _, _) in create_chunk_ixz_iter() {
            let chunk_diff_base = &base.chunks[i];
            let chunk_diff_squashing = &squashing.chunks[i];
            squashed_chunks[i] = match (chunk_diff_base, chunk_diff_squashing) {
                // BothNotExists and BothNotExists
                (ChunkWithTimestampDiff::BothNotExist, ChunkWithTimestampDiff::BothNotExist) => {
                    ChunkWithTimestampDiff::BothNotExist
                }

                // Create then Delete
                (ChunkWithTimestampDiff::Create(_, _), ChunkWithTimestampDiff::Delete(_, _)) => {
                    ChunkWithTimestampDiff::BothNotExist
                }

                // Delete then Create
                (
                    ChunkWithTimestampDiff::Delete(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Create(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Update(
                    *base_ts_diff + *squashing_ts_diff,
                    ChunkDiff::from_compare(
                        base_chunk_diff.get_old_text(),
                        squashing_chunk_diff.get_new_text(),
                    ),
                ),

                // BothNotExists then Create or Delete then BothNotExists
                (
                    ChunkWithTimestampDiff::BothNotExist,
                    ChunkWithTimestampDiff::Create(ts_diff, blob_diff),
                ) => ChunkWithTimestampDiff::Create(*ts_diff, blob_diff.clone()),
                (
                    ChunkWithTimestampDiff::Delete(ts_diff, blob_diff),
                    ChunkWithTimestampDiff::BothNotExist,
                ) => ChunkWithTimestampDiff::Delete(*ts_diff, blob_diff.clone()),

                // Update then Update
                (
                    ChunkWithTimestampDiff::Update(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Update(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Update(
                    *base_ts_diff + *squashing_ts_diff,
                    ChunkDiff::from_squash(base_chunk_diff, squashing_chunk_diff),
                ),

                // Create then Update or Update then Delete
                (
                    ChunkWithTimestampDiff::Create(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Update(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Create(
                    *base_ts_diff + *squashing_ts_diff,
                    BlobDiff::from_compare(
                        base_chunk_diff.get_old_text(),
                        &squashing_chunk_diff.patch(base_chunk_diff.get_new_text()),
                    ),
                ),
                (
                    ChunkWithTimestampDiff::Update(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Delete(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Delete(
                    *base_ts_diff + *squashing_ts_diff,
                    BlobDiff::from_compare(
                        &base_chunk_diff.revert(squashing_chunk_diff.get_old_text()),
                        squashing_chunk_diff.get_new_text(),
                    ),
                ),

                // NoChange with NoChange, Create, Delete, Update
                // (ChunkWithTimestampDiff::BothNotExist, ChunkWithTimestampDiff::NoChange)
                // | (ChunkWithTimestampDiff::Delete(_, _), ChunkWithTimestampDiff::NoChange)
                // | (ChunkWithTimestampDiff::NoChange, ChunkWithTimestampDiff::Create(_, _))
                // | (ChunkWithTimestampDiff::NoChange, ChunkWithTimestampDiff::BothNotExist) => {
                //     panic!("one diff is no change while another is a impossible diff",)
                // }
                (ChunkWithTimestampDiff::NoChange, ChunkWithTimestampDiff::NoChange) => {
                    ChunkWithTimestampDiff::NoChange
                }
                (
                    ChunkWithTimestampDiff::NoChange,
                    ChunkWithTimestampDiff::Delete(ts_diff, chunk_diff),
                ) => ChunkWithTimestampDiff::Delete(*ts_diff, chunk_diff.clone()),
                (
                    ChunkWithTimestampDiff::NoChange,
                    ChunkWithTimestampDiff::Update(ts_diff, chunk_diff),
                ) => ChunkWithTimestampDiff::Update(*ts_diff, chunk_diff.clone()),
                (
                    ChunkWithTimestampDiff::Create(ts_diff, chunk_diff),
                    ChunkWithTimestampDiff::NoChange,
                ) => ChunkWithTimestampDiff::Create(*ts_diff, chunk_diff.clone()),
                (
                    ChunkWithTimestampDiff::Update(ts_diff, chunk_diff),
                    ChunkWithTimestampDiff::NoChange,
                ) => ChunkWithTimestampDiff::Update(*ts_diff, chunk_diff.clone()),

                // else: panics
                (base_diff, squashing_diff) => {
                    panic!(
                        "base diff {}, while squashing diff {}, which is impossible",
                        base_diff.get_description(),
                        squashing_diff.get_description()
                    )
                }
            };
        }
        Self {
            chunks: squashed_chunks,
        }
    }

    fn patch(&self, old: &[u8]) -> Vec<u8> {
        let reader = MCAReader::from_bytes(old).unwrap();
        let mut builder = MCABuilder::new();
        let mut chunks_holder = Vec::with_capacity(1024);
        for (i, x, z) in create_chunk_ixz_iter() {
            let old_chunk = reader.get_chunk_lazily(x, z);
            let chunk_diff = &self.chunks[i];
            let new_chunk = match (old_chunk, chunk_diff) {
                // LazyChunk::Unloaded is invalid
                (LazyChunk::Unloaded, _) => panic!("old chunk is unloaded"),

                // LazyChunk::NotExists accepts ChunkWithTimestampDiff::{BothNotExist, Create}
                (LazyChunk::NotExists, ChunkWithTimestampDiff::BothNotExist) => None,
                (
                    LazyChunk::NotExists,
                    ChunkWithTimestampDiff::Create(timestamp_diff, chunk_diff),
                ) => {
                    assert!(*timestamp_diff != 0);
                    Some(ChunkWithTimestamp {
                        timestamp: *timestamp_diff as u32,
                        nbt: chunk_diff.patch(&[]),
                    })
                }
                (LazyChunk::NotExists, diff) => panic!(
                    "old chunk not exists, while chunk diff {}, which is impossible",
                    diff.get_description()
                ),

                // LazyChunk::Some accepts ChunkWithTimestampDiff::{Delete, Update}
                (LazyChunk::Some(_), ChunkWithTimestampDiff::Delete(_, _)) => None,
                (
                    LazyChunk::Some(old_chunk),
                    ChunkWithTimestampDiff::Update(timestamp_diff, chunk_diff),
                ) => Some(ChunkWithTimestamp {
                    timestamp: old_chunk
                        .timestamp
                        .checked_add_signed(*timestamp_diff)
                        .expect("timestamp overflowed"),
                    nbt: chunk_diff.patch(&old_chunk.nbt),
                }),
                (LazyChunk::Some(_), diff) => panic!(
                    "old chunk exists, while chunk diff {}, which is impossible",
                    diff.get_description()
                ),
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
            let new_chunk = reader.get_chunk_lazily(x, z);
            let chunk_diff = &self.chunks[i];
            let old_chunk = match (chunk_diff, new_chunk) {
                // LazyChunk::Unloaded
                (_, LazyChunk::Unloaded) => panic!("new chunk is unloaded"),

                // ChunkWithTimestampDiff::{Delete, BothNotExist} accept LazyChunk::NotExists
                (ChunkWithTimestampDiff::BothNotExist, LazyChunk::NotExists) => None,
                (
                    ChunkWithTimestampDiff::Delete(timestamp_diff, chunk_diff),
                    LazyChunk::NotExists,
                ) => Some(ChunkWithTimestamp {
                    timestamp: (-*timestamp_diff) as u32,
                    nbt: chunk_diff.revert(&[]),
                }),
                (diff, LazyChunk::NotExists) => panic!(
                    "diff {}, while new chunk not exists, which is impossible",
                    diff.get_description()
                ),

                // ChunkWithTimestampDiff::{Create, Update} accepts LazyChunk::Some
                (ChunkWithTimestampDiff::Create(_, _), LazyChunk::Some(_)) => None,
                (
                    ChunkWithTimestampDiff::Update(timestamp_diff, chunk_diff),
                    LazyChunk::Some(new_chunk),
                ) => Some(ChunkWithTimestamp {
                    timestamp: new_chunk
                        .timestamp
                        .checked_add_signed(-*timestamp_diff)
                        .expect("timestamp overflowed"),
                    nbt: chunk_diff.revert(&new_chunk.nbt),
                }),
                (diff, LazyChunk::Some(_)) => panic!(
                    "diff {}, while new chunk exists, which is impossible",
                    diff.get_description()
                ),
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
impl Serde for McaDiff {
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
    use fastnbt::Value;
    use rand::prelude::*;

    use super::*;
    use crate::{
        mca::{LazyChunk, MCAReader},
        util::rearranged_nbt,
    };

    #[test]
    fn test_mca_timestamp_nbt() {
        // TODO: replace test mca files
        let reader_old = MCAReader::from_file("./resources/mca/r.1.2.20250511.mca", false).unwrap();
        let reader_new = MCAReader::from_file("./resources/mca/r.1.2.20250512.mca", false).unwrap();
        let mut ts_changed_chunk_count = 0;
        let mut ts_unchanged_chunk_count = 0;
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
                ts_unchanged_chunk_count += 1;
                assert_eq!(nbt_old, nbt_new);
            } else {
                ts_changed_chunk_count += 1;
                assert_ne!(nbt_old, nbt_new);
            }
        }
        // assert!(ts_changed_chunk_count > 20);
        // assert!(ts_unchanged_chunk_count > 20);
    }
    fn build_test_mca(path: &str, chunks: usize, rng: &mut StdRng) -> Vec<u8> {
        let avaliable_indexes: Vec<_> = create_chunk_ixz_iter().collect();
        let reader = MCAReader::from_file(path, false).unwrap();
        let mut builder = MCABuilder::new();
        for (_, x, z) in avaliable_indexes.into_iter().choose_multiple(rng, chunks) {
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
            if chunk_a.is_some() && chunk_b.is_some() {
                let ChunkWithTimestamp {
                    timestamp: ts_a,
                    nbt: nbt_a,
                } = chunk_a.unwrap();
                let ChunkWithTimestamp {
                    timestamp: ts_b,
                    nbt: nbt_b,
                } = chunk_b.unwrap();
                assert_eq!(ts_a, ts_b);
                assert_eq!(
                    fastnbt::from_bytes::<Value>(nbt_a),
                    fastnbt::from_bytes::<Value>(nbt_b)
                );
            } else {
                assert_eq!(chunk_a, chunk_b);
            }
        }
    }
    mod test_in_continuous_data {
        use super::*;
        #[test]
        fn test_diff_patch_revert() {
            let paths = [
                "./resources/mca/r.1.2.20250511.mca",
                "./resources/mca/r.1.2.20250512.mca",
                "./resources/mca/r.1.2.20250513.mca",
                "./resources/mca/r.1.2.20250514.mca",
                "./resources/mca/r.1.2.20250515.mca",
                "./resources/mca/r.1.2.20250516.mca",
            ];
            let seed = 114514;
            let rng = StdRng::seed_from_u64(seed);
            for path_old_new in paths.windows(2) {
                let old = build_test_mca(path_old_new[0], 100, &mut rng.clone());
                let new = build_test_mca(path_old_new[1], 100, &mut rng.clone());
                let diff = McaDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);

                assert_mca_eq(&new, &patched_old);
                assert_mca_eq(&old, &reverted_new);
            }
        }
        #[test]
        fn test_diff_squash() {
            let paths = [
                "./resources/mca/r.1.2.20250511.mca",
                "./resources/mca/r.1.2.20250512.mca",
                "./resources/mca/r.1.2.20250513.mca",
                "./resources/mca/r.1.2.20250514.mca",
                "./resources/mca/r.1.2.20250515.mca",
                "./resources/mca/r.1.2.20250516.mca",
            ];
            let seed = 114514;
            let rng = StdRng::seed_from_u64(seed);
            for path_old_new in paths.windows(3) {
                let v0 = build_test_mca(path_old_new[0], 50, &mut rng.clone());
                let v1 = build_test_mca(path_old_new[1], 50, &mut rng.clone());
                let v2 = build_test_mca(path_old_new[2], 50, &mut rng.clone());
                let diff_v01 = McaDiff::from_compare(&v0, &v1);
                let diff_v12 = McaDiff::from_compare(&v1, &v2);
                let squashed_diff = McaDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);

                assert_mca_eq(&v2, &patched_v0);
                assert_mca_eq(&v0, &reverted_v2);
            }
        }
    }
    mod test_in_noncontinuous_data {
        use super::*;
        #[test]
        fn test_diff_patch_revert() {
            let paths = [
                "./resources/mca/r.1.2.20250511.mca",
                "./resources/mca/r.1.2.20250512.mca",
                "./resources/mca/r.1.2.20250513.mca",
                "./resources/mca/r.1.2.20250514.mca",
                "./resources/mca/r.1.2.20250515.mca",
                "./resources/mca/r.1.2.20250516.mca",
            ];
            let seed = 114514;
            let mut rng = StdRng::seed_from_u64(seed);
            for path_old_new in paths.windows(2) {
                let old = build_test_mca(path_old_new[0], 100, &mut rng);
                let new = build_test_mca(path_old_new[1], 100, &mut rng);
                let diff = McaDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);

                assert_mca_eq(&new, &patched_old);
                assert_mca_eq(&old, &reverted_new);
            }
        }
        #[test]
        fn test_diff_squash() {
            let paths = [
                "./resources/mca/r.1.2.20250511.mca",
                "./resources/mca/r.1.2.20250512.mca",
                "./resources/mca/r.1.2.20250513.mca",
                "./resources/mca/r.1.2.20250514.mca",
                "./resources/mca/r.1.2.20250515.mca",
                "./resources/mca/r.1.2.20250516.mca",
            ];
            let seed = 114514;
            let mut rng = StdRng::seed_from_u64(seed);
            for path_old_new in paths.windows(3) {
                let v0 = build_test_mca(path_old_new[0], 50, &mut rng);
                rng.next_u32();
                let v1 = build_test_mca(path_old_new[1], 50, &mut rng);
                rng.next_u32();
                let v2 = build_test_mca(path_old_new[2], 50, &mut rng);
                rng.next_u32();

                let diff_v01 = McaDiff::from_compare(&v0, &v1);
                let diff_v12 = McaDiff::from_compare(&v1, &v2);
                let squashed_diff = McaDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);

                assert_mca_eq(&v2, &patched_v0);
                assert_mca_eq(&v0, &reverted_v2);
            }
        }
    }
}
