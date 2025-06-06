use bincode::{Decode, Encode};
use log::{Level, log_enabled};
use rayon::{ThreadPoolBuilder, prelude::*};

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::compress::CompressionType;
use crate::{
    diff::{Diff, base::BlobDiff, nbt::ChunkDiff},
    mca::{ChunkWithTimestamp, LazyChunk, MCABuilder, MCAReader},
    util::{create_chunk_ixz_iter, fastnbt_deserialize as de, fastnbt_serialize as ser},
};

#[derive(Debug, Clone, Encode, Decode)]
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
#[derive(Debug, Clone, Encode, Decode)]
pub struct MCADiff {
    chunks: Vec<ChunkWithTimestampDiff>,
}

impl Diff<Vec<u8>> for MCADiff {
    fn from_compare(old: &Vec<u8>, new: &Vec<u8>) -> Self
    where
        Self: Sized,
    {
        log::trace!("from_compare()...");

        let reader_old = Arc::new(MCAReader::from_bytes(old).unwrap());
        let reader_new = Arc::new(MCAReader::from_bytes(new).unwrap());
        let enable_cost_stat = log_enabled!(Level::Debug);

        let mut chunks = vec![ChunkWithTimestampDiff::BothNotExist; 1024];
        let ixz_list = create_chunk_ixz_iter().collect::<Vec<_>>(); // shuffle may helpful or helpless

        let pool = ThreadPoolBuilder::new()
            .num_threads(crate::config::get_config().threads)
            .build()
            .unwrap();

        let process_chunk = |(i, x, z): &(usize, usize, usize)| {
            log::trace!("compare chunk i: {}", i);
            let timing_start = if enable_cost_stat {
                Some(Instant::now())
            } else {
                None
            };

            let old_ts = reader_old.get_timestamp(*x, *z);
            let new_ts = reader_new.get_timestamp(*x, *z);
            let ts_diff = new_ts as i32 - old_ts as i32;

            let chunk = match (old_ts, new_ts, ts_diff) {
                (0, 0, _) => ChunkWithTimestampDiff::BothNotExist,
                (_, _, 0) => ChunkWithTimestampDiff::NoChange,
                _ => {
                    let old = reader_old.get_chunk_lazily(*x, *z);
                    let new = reader_new.get_chunk_lazily(*x, *z);
                    match (old, new) {
                        (LazyChunk::Unloaded, _) => panic!("old chunk is unloaded"),
                        (_, LazyChunk::Unloaded) => panic!("new chunk is unloaded"),
                        (LazyChunk::NotExists, LazyChunk::NotExists) => {
                            ChunkWithTimestampDiff::BothNotExist
                        }
                        (LazyChunk::NotExists, LazyChunk::Some(chunk)) => {
                            ChunkWithTimestampDiff::Create(
                                chunk.timestamp as i32 - 0,
                                BlobDiff::from_compare(&Vec::with_capacity(0), &chunk.nbt),
                            )
                        }
                        (LazyChunk::Some(chunk), LazyChunk::NotExists) => {
                            ChunkWithTimestampDiff::Delete(
                                0 - chunk.timestamp as i32,
                                BlobDiff::from_compare(&chunk.nbt, &Vec::with_capacity(0)),
                            )
                        }
                        (LazyChunk::Some(chunk_old), LazyChunk::Some(chunk_new)) => {
                            let ts_diff = chunk_new.timestamp as i32 - chunk_old.timestamp as i32;
                            if ts_diff == 0 {
                                ChunkWithTimestampDiff::NoChange
                            } else {
                                ChunkWithTimestampDiff::Update(
                                    ts_diff,
                                    ChunkDiff::from_compare(
                                        &de(&chunk_old.nbt),
                                        &de(&chunk_new.nbt),
                                    ),
                                )
                            }
                        }
                    }
                }
            };

            let cost = timing_start.map(|s| s.elapsed());
            (*i, chunk, cost, *x, *z)
        };

        let mut results: Vec<_> = vec![];
        pool.install(|| {
            results = ixz_list.par_iter().map(process_chunk).collect();
        });

        let mut chunk_costs = if enable_cost_stat {
            results
                .iter()
                .filter_map(|&(i, _, cost, x, z)| cost.map(|c| (c, i, x, z)))
                .collect()
        } else {
            Vec::new()
        };

        for (i, chunk, _, _, _) in results {
            chunks[i] = chunk;
        }

        if enable_cost_stat {
            chunk_costs.sort_by(|a, b| b.0.cmp(&a.0));
            let total_cost = chunk_costs.iter().map(|(d, _, _, _)| *d).sum::<Duration>();
            log::debug!(
                "chunk time costs stat:\n- total {:?}\n- avg   {:?}\n- p100  {:?}\n- p99   {:?}\n- p95   {:?}\n- p50   {:?}",
                total_cost,
                total_cost / 1024,
                chunk_costs[0].0,
                chunk_costs[10].0,
                chunk_costs[51].0,
                chunk_costs[512].0,
            );
            log::debug!(
                "chunk time costs top 8:\n{}",
                chunk_costs[0..8]
                    .iter()
                    .map(|(d, i, x, z)| format!("- chunk {} ({}, {}) (cost {:?})", i, x, z, d))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }

        Self { chunks }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        let mut squashed_chunks = vec![const { ChunkWithTimestampDiff::BothNotExist }; 1024];
        for (i, _, _) in create_chunk_ixz_iter() {
            let chunk_diff_base = &base.chunks[i];
            let chunk_diff_squashing = &squashing.chunks[i];
            squashed_chunks[i] = match (chunk_diff_base, chunk_diff_squashing) {
                // BothNotExist and BothNotExist
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
                        &de(base_chunk_diff.get_old_text()),
                        &de(squashing_chunk_diff.get_new_text()),
                    ),
                ),

                // BothNotExist then Create or Delete then BothNotExist
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
                        &ser(&squashing_chunk_diff.patch(&de(base_chunk_diff.get_new_text()))),
                    ),
                ),
                (
                    ChunkWithTimestampDiff::Update(base_ts_diff, base_chunk_diff),
                    ChunkWithTimestampDiff::Delete(squashing_ts_diff, squashing_chunk_diff),
                ) => ChunkWithTimestampDiff::Delete(
                    *base_ts_diff + *squashing_ts_diff,
                    BlobDiff::from_compare(
                        &ser(&base_chunk_diff.revert(&de(squashing_chunk_diff.get_old_text()))),
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

    fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
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
                        nbt: chunk_diff.patch(&Vec::with_capacity(0)),
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
                    nbt: ser(&chunk_diff.patch(&de(&old_chunk.nbt))),
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

    fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
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
                    nbt: chunk_diff.revert(&Vec::with_capacity(0)),
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
                    nbt: ser(&chunk_diff.revert(&de(&new_chunk.nbt))),
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
#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::{
        config::{Config, with_test_config},
        mca::{LazyChunk, MCAReader},
        util::test::{all_file_cp2_iter, all_file_cp3_iter, assert_mca_eq, rearranged_nbt},
    };

    use super::*;

    static TEST_CONFIG: Config = Config {
        log_config: crate::config::LogConfig::NoLog,
        threads: 16,
    };

    #[test]
    #[ignore = "replace test mca files"]
    fn test_mca_timestamp_nbt() {
        with_test_config(TEST_CONFIG.clone(), || {
            let reader_old = MCAReader::from_file(
                &PathBuf::from(
                    "./resources/test-payload/region/mca/hairlessvillager-0/20250511.mca",
                ),
                false,
            )
            .unwrap();
            let reader_new = MCAReader::from_file(
                &PathBuf::from(
                    "./resources/test-payload/region/mca/hairlessvillager-0/20250512.mca",
                ),
                false,
            )
            .unwrap();
            let mut ts_changed_chunk_count = 0;
            let mut ts_unchanged_chunk_count = 0;
            for (_, x, z) in create_chunk_ixz_iter() {
                let (timestamp_old, nbt_old) = match reader_old.get_chunk_lazily(x, z) {
                    LazyChunk::Some(chunk) => {
                        (chunk.timestamp, rearranged_nbt(&chunk.nbt).unwrap())
                    }
                    _ => panic!("chunk should loaded"),
                };
                let (timestamp_new, nbt_new) = match reader_new.get_chunk_lazily(x, z) {
                    LazyChunk::Some(chunk) => {
                        (chunk.timestamp, rearranged_nbt(&chunk.nbt).unwrap())
                    }
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
            assert!(ts_changed_chunk_count > 20);
            assert!(ts_unchanged_chunk_count > 20);
        });
    }
    #[test]
    fn test_diff_patch_revert() {
        with_test_config(TEST_CONFIG.clone(), || {
            for paths in all_file_cp2_iter(crate::FileType::RegionMca) {
                for (old_path, new_path) in paths {
                    let old = fs::read(old_path).unwrap();
                    let new = fs::read(new_path).unwrap();
                    let diff = MCADiff::from_compare(&old, &new);
                    let patched_old = diff.patch(&old);
                    let reverted_new = diff.revert(&new);
                    assert_mca_eq(&new, &patched_old);
                    assert_mca_eq(&old, &reverted_new);
                }
            }
        });
    }
    #[test]
    fn test_diff_squash() {
        with_test_config(TEST_CONFIG.clone(), || {
            for paths in all_file_cp3_iter(crate::FileType::RegionMca) {
                for (v0_path, v1_path, v2_path) in paths {
                    let v0 = fs::read(v0_path).unwrap();
                    let v1 = fs::read(v1_path).unwrap();
                    let v2 = fs::read(v2_path).unwrap();
                    let diff_v01 = MCADiff::from_compare(&v0, &v1);
                    let diff_v12 = MCADiff::from_compare(&v1, &v2);
                    let squashed_diff = MCADiff::from_squash(&diff_v01, &diff_v12);
                    let patched_v0 = squashed_diff.patch(&v0);
                    let reverted_v2 = squashed_diff.revert(&v2);
                    assert_mca_eq(&v2, &patched_v0);
                    assert_mca_eq(&v0, &reverted_v2);
                }
            }
        });
    }
    #[test]
    #[ignore = "use benchmark test"]
    fn test_time_cost() {
        // The next performance hotspot is the diff of sections, but since the
        // current performance is already good enough, I don't plan to
        // optimize this area in the near future.
        with_test_config(TEST_CONFIG.clone(), || {
            log::debug!("reading files...");
            let a = fs::read("./resources/mca/r.1.2.20250515.mca").unwrap();
            let b = fs::read("./resources/mca/r.1.2.20250516.mca").unwrap();
            MCADiff::from_compare(&a, &b);
        });
    }
}
