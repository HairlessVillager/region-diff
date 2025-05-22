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

#[derive(Debug)]
enum ChunkWithTimestampDiff {
    NotExists,
    Minor(i32, ChunkDiff),
    Major(i32, BlobDiff),
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
                    chunk.timestamp as i32,
                    BlobDiff::from_compare(&[], &chunk.nbt),
                ),
                (LazyChunk::Some(chunk), LazyChunk::NotExists) => ChunkWithTimestampDiff::Major(
                    -(chunk.timestamp as i32),
                    BlobDiff::from_compare(&chunk.nbt, &[]),
                ),
                (LazyChunk::Some(chunk_old), LazyChunk::Some(chunk_new)) => {
                    ChunkWithTimestampDiff::Minor(
                        chunk_new.timestamp as i32 - chunk_old.timestamp as i32,
                        ChunkDiff::from_compare(&chunk_old.nbt, &chunk_new.nbt),
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
        let mut squashed_chunks = [const { ChunkWithTimestampDiff::NotExists }; 1024];
        for (i, _, _) in create_chunk_ixz_iter() {
            let chunk_diff_base = &base.chunks[i];
            let chunk_diff_squashing = &squashing.chunks[i];
            squashed_chunks[i] = match (chunk_diff_base, chunk_diff_squashing) {
                (ChunkWithTimestampDiff::NotExists, ChunkWithTimestampDiff::NotExists) => {
                    ChunkWithTimestampDiff::NotExists
                }
                (
                    ChunkWithTimestampDiff::NotExists,
                    ChunkWithTimestampDiff::Major(ts_diff, blob_diff),
                ) => ChunkWithTimestampDiff::Major(
                    *ts_diff,
                    BlobDiff::from_compare(&[], blob_diff.get_new_text()),
                ),
                (
                    ChunkWithTimestampDiff::Minor(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Minor(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Minor(
                    *base_ts_diff + *squashing_ts_diff,
                    ChunkDiff::from_squash(base_chunk_diff, squashing_chunk_diff),
                ),
                (
                    ChunkWithTimestampDiff::Minor(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Major(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Major(
                    *base_ts_diff + *squashing_ts_diff,
                    BlobDiff::from_compare(
                        &base_chunk_diff.revert(squashing_chunk_diff.get_old_text()),
                        squashing_chunk_diff.get_new_text(),
                    ),
                ),
                (
                    ChunkWithTimestampDiff::Major(ts_diff, blob_diff),
                    ChunkWithTimestampDiff::NotExists,
                ) => ChunkWithTimestampDiff::Major(
                    -*ts_diff,
                    BlobDiff::from_compare(blob_diff.get_old_text(), &[]),
                ),
                (
                    ChunkWithTimestampDiff::Major(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Minor(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Major(
                    *base_ts_diff + *squashing_ts_diff,
                    BlobDiff::from_compare(
                        base_chunk_diff.get_old_text(),
                        &squashing_chunk_diff.patch(base_chunk_diff.get_new_text()),
                    ),
                ),
                (
                    ChunkWithTimestampDiff::Major(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Major(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Major(
                    *base_ts_diff + *squashing_ts_diff,
                    BlobDiff::from_squash(base_chunk_diff, squashing_chunk_diff),
                ),
                (ChunkWithTimestampDiff::NotExists, ChunkWithTimestampDiff::Minor(_, _))
                | (ChunkWithTimestampDiff::Minor(_, _), ChunkWithTimestampDiff::NotExists) => {
                    panic!(
                        "one of the diff not exists, with another is minor diff, which is impossible"
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
                    timestamp: *timestamp_diff as u32,
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
                    timestamp: *timestamp_diff as u32,
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
    // TODO: test_in_continuous_data & test_in_noncontinuous_data
}
