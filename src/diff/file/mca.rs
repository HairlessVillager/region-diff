use crate::compress::CompressionType;
use crate::util::IXZ;
use crate::{
    diff::{Diff, base::BlobDiff, nbt::ChunkDiff},
    mca::{ChunkWithTimestamp, LazyChunk, MCABuilder, MCAReader},
    util::{create_chunk_ixz_iter, fastnbt_deserialize as de, fastnbt_serialize as ser},
};
use bincode::{Decode, Encode};
use log::{Level, log_enabled};
use rayon::{ThreadPoolBuilder, prelude::*};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

fn parallel_process<F, R>(process_func: F) -> Vec<(IXZ, R, Option<Duration>)>
where
    F: Fn(IXZ) -> R + Sync + Send,
    R: Send,
{
    let ixz_list = create_chunk_ixz_iter().collect::<Vec<_>>();
    // ixz_list.shuffle(...);
    // NOTE: may enhance or degrade performance. There are currently no clear
    // evaluation results supporting whether it should be enabled, so it is
    // not enabled for the time being.

    let pool = ThreadPoolBuilder::new()
        .num_threads(crate::config::get_config().threads)
        .build()
        .unwrap();

    let results = pool.install(|| {
        ixz_list
            .par_iter()
            .map(|&ixz| {
                let start = enable_cost_stat().then_some(Instant::now());
                let result = process_func(ixz);
                let cost = start.map(|s| s.elapsed());
                (ixz, result, cost)
            })
            .collect::<Vec<_>>()
    });

    results
}

fn log_cost_statistics<R>(result: &[(IXZ, R, Option<Duration>)]) {
    let len = result.len();
    let mut sorted_costs = result
        .iter()
        .map(|(ixz, _, duration)| (ixz, duration))
        .collect::<Vec<_>>();
    sorted_costs.sort_by(|(_, a), (_, b)| a.cmp(b));

    let total_cost = sorted_costs.iter().map(|e| e.1.unwrap()).sum::<Duration>();
    log::debug!(
        "time costs stat:\n- total {:?}\n- avg   {:?}\n- p100  {:?}\n- p99   {:?}\n- p95   {:?}\n- p50   {:?}",
        total_cost,
        total_cost / len as u32,
        sorted_costs[0].0,
        sorted_costs.get(len / 100).map(|c| c.0).unwrap(),
        sorted_costs.get(len / 20).map(|c| c.0).unwrap(),
        sorted_costs.get(len / 2).map(|c| c.0).unwrap(),
    );

    log::debug!(
        "time costs top 8:\n{}",
        sorted_costs[0..8]
            .iter()
            .map(|((i, x, z), d)| format!("- chunk {} ({}, {}) (cost {:?})", i, x, z, d.unwrap()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn enable_cost_stat() -> bool {
    log_enabled!(Level::Debug)
}

impl Diff<Vec<u8>> for MCADiff {
    fn from_compare(old: &Vec<u8>, new: &Vec<u8>) -> Self {
        log::trace!("from_compare()...");
        let reader_old = Arc::new(MCAReader::from_bytes(old).unwrap());
        let reader_new = Arc::new(MCAReader::from_bytes(new).unwrap());

        let results = parallel_process(|(_, x, z)| {
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
                                chunk.timestamp as i32,
                                BlobDiff::from_compare(&Vec::new(), &chunk.nbt),
                            )
                        }
                        (LazyChunk::Some(chunk), LazyChunk::NotExists) => {
                            ChunkWithTimestampDiff::Delete(
                                -(chunk.timestamp as i32),
                                BlobDiff::from_compare(&chunk.nbt, &Vec::new()),
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
            chunk
        });

        if enable_cost_stat() {
            log_cost_statistics(&results);
        }

        let mut chunks = vec![ChunkWithTimestampDiff::BothNotExist; 1024];
        for ((i, _, _), chunk, _) in results {
            chunks[i] = chunk;
        }

        Self { chunks }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self {
        log::trace!("from_squash()...");

        let results = parallel_process(|(i, _, _)| {
            let base_diff = &base.chunks[i];
            let squashing_diff = &squashing.chunks[i];

            let squashed = match (base_diff, squashing_diff) {
                (ChunkWithTimestampDiff::BothNotExist, ChunkWithTimestampDiff::BothNotExist) => {
                    ChunkWithTimestampDiff::BothNotExist
                }
                (ChunkWithTimestampDiff::Create(_, _), ChunkWithTimestampDiff::Delete(_, _)) => {
                    ChunkWithTimestampDiff::BothNotExist
                }
                (
                    ChunkWithTimestampDiff::Delete(base_ts, base_diff),
                    ChunkWithTimestampDiff::Create(squashing_ts, squashing_diff),
                ) => ChunkWithTimestampDiff::Update(
                    base_ts + squashing_ts,
                    ChunkDiff::from_compare(
                        &de(base_diff.get_old_text()),
                        &de(squashing_diff.get_new_text()),
                    ),
                ),
                (
                    ChunkWithTimestampDiff::BothNotExist,
                    ChunkWithTimestampDiff::Create(ts_diff, blob_diff),
                ) => ChunkWithTimestampDiff::Create(*ts_diff, blob_diff.clone()),
                (
                    ChunkWithTimestampDiff::Delete(ts_diff, blob_diff),
                    ChunkWithTimestampDiff::BothNotExist,
                ) => ChunkWithTimestampDiff::Delete(*ts_diff, blob_diff.clone()),
                (
                    ChunkWithTimestampDiff::Update(base_ts, base_diff),
                    ChunkWithTimestampDiff::Update(squashing_ts, squashing_diff),
                ) => ChunkWithTimestampDiff::Update(
                    base_ts + squashing_ts,
                    ChunkDiff::from_squash(base_diff, squashing_diff),
                ),
                (
                    ChunkWithTimestampDiff::Create(base_ts, base_diff),
                    ChunkWithTimestampDiff::Update(squashing_ts, squashing_diff),
                ) => ChunkWithTimestampDiff::Create(
                    base_ts + squashing_ts,
                    BlobDiff::from_compare(
                        base_diff.get_old_text(),
                        &ser(&squashing_diff.patch(&de(base_diff.get_new_text()))),
                    ),
                ),
                (
                    ChunkWithTimestampDiff::Update(base_ts, base_diff),
                    ChunkWithTimestampDiff::Delete(squashing_ts, squashing_diff),
                ) => ChunkWithTimestampDiff::Delete(
                    base_ts + squashing_ts,
                    BlobDiff::from_compare(
                        &ser(&base_diff.revert(&de(squashing_diff.get_old_text()))),
                        squashing_diff.get_new_text(),
                    ),
                ),
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
                (base, squashing) => panic!(
                    "Impossible diff combination: base={}, squashing={}",
                    base.get_description(),
                    squashing.get_description()
                ),
            };
            squashed
        });

        if enable_cost_stat() {
            log_cost_statistics(&results);
        }

        let mut squashed_chunks = vec![ChunkWithTimestampDiff::BothNotExist; 1024];
        for ((i, _, _), chunk, _) in results {
            squashed_chunks[i] = chunk;
        }

        Self {
            chunks: squashed_chunks,
        }
    }

    fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
        log::trace!("patch()...");
        let reader = Arc::new(MCAReader::from_bytes(old).unwrap());
        let enable_cost_stat = log_enabled!(Level::Debug);

        let results = parallel_process(|(_, x, z)| {
            let old_chunk = reader.get_chunk_lazily(x, z);
            let chunk_diff = &self.chunks[z * 32 + x];

            let new_chunk = match (old_chunk, chunk_diff) {
                (LazyChunk::Unloaded, _) => panic!("old chunk is unloaded"),
                (LazyChunk::NotExists, ChunkWithTimestampDiff::BothNotExist) => None,
                (
                    LazyChunk::NotExists,
                    ChunkWithTimestampDiff::Create(timestamp_diff, chunk_diff),
                ) => {
                    assert!(*timestamp_diff != 0);
                    Some(ChunkWithTimestamp {
                        timestamp: *timestamp_diff as u32,
                        nbt: chunk_diff.patch(&Vec::new()),
                    })
                }
                (LazyChunk::NotExists, diff) => panic!(
                    "Invalid diff for non-existing chunk: {}",
                    diff.get_description()
                ),
                (LazyChunk::Some(_), ChunkWithTimestampDiff::Delete(_, _)) => None,
                (
                    LazyChunk::Some(old_chunk),
                    ChunkWithTimestampDiff::Update(timestamp_diff, chunk_diff),
                ) => Some(ChunkWithTimestamp {
                    timestamp: old_chunk
                        .timestamp
                        .checked_add_signed(*timestamp_diff)
                        .expect("timestamp overflow"),
                    nbt: ser(&chunk_diff.patch(&de(&old_chunk.nbt))),
                }),
                (LazyChunk::Some(old_chunk),ChunkWithTimestampDiff::NoChange)=>Some(old_chunk.clone()),
                (LazyChunk::Some(_), diff) => panic!(
                    "Invalid diff for existing chunk: {}",
                    diff.get_description()
                ),
            };
            new_chunk
        });

        if enable_cost_stat {
            log_cost_statistics(&results);
        }

        let mut builder = MCABuilder::new();
        for ((_, x, z), new_chunk, _) in &results {
            if let Some(chunk) = new_chunk {
                builder.set_chunk(*x, *z, &chunk);
            }
        }

        builder.to_bytes(CompressionType::Zlib)
    }

    fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
        log::trace!("revert()...");
        let reader = Arc::new(MCAReader::from_bytes(new).unwrap());
        let enable_cost_stat = log_enabled!(Level::Debug);

        let results = parallel_process(|(_, x, z)| {
            let new_chunk = reader.get_chunk_lazily(x, z);
            let chunk_diff = &self.chunks[z * 32 + x];

            let old_chunk = match (chunk_diff, new_chunk) {
                (_, LazyChunk::Unloaded) => panic!("new chunk is unloaded"),
                (ChunkWithTimestampDiff::BothNotExist, LazyChunk::NotExists) => None,
                (
                    ChunkWithTimestampDiff::Delete(timestamp_diff, chunk_diff),
                    LazyChunk::NotExists,
                ) => Some(ChunkWithTimestamp {
                    timestamp: (-*timestamp_diff) as u32,
                    nbt: chunk_diff.revert(&Vec::new()),
                }),
                (diff, LazyChunk::NotExists) => panic!(
                    "Invalid diff for non-existing chunk: {}",
                    diff.get_description()
                ),
                (ChunkWithTimestampDiff::Create(_, _), LazyChunk::Some(_)) => None,
                (
                    ChunkWithTimestampDiff::Update(timestamp_diff, chunk_diff),
                    LazyChunk::Some(new_chunk),
                ) => Some(ChunkWithTimestamp {
                    timestamp: new_chunk
                        .timestamp
                        .checked_add_signed(-*timestamp_diff)
                        .expect("timestamp overflow"),
                    nbt: ser(&chunk_diff.revert(&de(&new_chunk.nbt))),
                }),
                (ChunkWithTimestampDiff::NoChange, LazyChunk::Some(new_chunk)) => Some(new_chunk.clone()),
                (diff, LazyChunk::Some(_)) => panic!(
                    "Invalid diff for existing chunk: {}",
                    diff.get_description()
                ),
            };
            old_chunk
        });

        if enable_cost_stat {
            log_cost_statistics(&results);
        }

        let mut builder = MCABuilder::new();
        for ((_, x, z), old_chunk, _) in &results {
            if let Some(chunk) = old_chunk {
                builder.set_chunk(*x, *z, &chunk);
            }
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
        util::test::{all_file_iter, assert_mca_eq, rearranged_nbt},
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
            for paths in all_file_iter(crate::FileType::RegionMca) {
                for window in paths.collect::<Vec<_>>().windows(2) {
                    let old = fs::read(window[0].clone()).unwrap();
                    let new = fs::read(window[1].clone()).unwrap();
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
            for paths in all_file_iter(crate::FileType::RegionMca) {
                for window in paths.collect::<Vec<_>>().windows(3) {
                    let v0 = fs::read(window[0].clone()).unwrap();
                    let v1 = fs::read(window[1].clone()).unwrap();
                    let v2 = fs::read(window[2].clone()).unwrap();
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
