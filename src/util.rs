pub type IXZ = (usize, usize, usize);
pub fn create_chunk_ixz_iter() -> impl Iterator<Item = IXZ> {
    (0..32).flat_map(|z| {
        (0..32).map(move |x| {
            let i = x + 32 * z;
            (i, x, z)
        })
    })
}

pub mod nbt_serde {
    pub fn ser(v: &fastnbt::Value) -> Vec<u8> {
        fastnbt::to_bytes(v).expect("Failed to serialize NBT data")
    }
    pub fn de(input: &[u8]) -> fastnbt::Value {
        fastnbt::from_bytes(input).expect("Failed to deserialize NBT data")
    }
}

pub mod serde {
    use bincode::{
        Decode, Encode,
        config::{BigEndian, Configuration},
        decode_from_slice, encode_to_vec,
    };

    static CONFIG: Configuration<BigEndian> = bincode::config::standard()
        .with_big_endian()
        .with_variable_int_encoding();

    pub fn ser<T: Encode>(val: T) -> Vec<u8> {
        encode_to_vec(val, CONFIG.clone()).expect("Failed to serialize object to bytes")
    }
    pub fn de<T: Decode<()>>(data: &Vec<u8>) -> T {
        decode_from_slice(data, CONFIG.clone())
            .map(|(de, _)| de)
            .expect("Failed to deserialize object from bytes")
    }
}

pub mod parallel {
    use std::{
        fmt::Debug,
        time::{Duration, Instant},
    };

    use rayon::{ThreadPoolBuilder, prelude::*};

    pub fn parallel_process<I, O, G, F>(
        task_generator: G,
        process_func: F,
    ) -> Vec<(I, O, Option<Duration>)>
    where
        I: Send + Debug,
        O: Send,
        G: Iterator<Item = I> + ParallelBridge + Send,
        F: Fn(&I) -> O + Sync + Send,
    {
        let pool = ThreadPoolBuilder::new()
            .num_threads(crate::config::get_config().threads)
            .build()
            .expect("Failed to build thread pool");

        pool.install(|| {
            task_generator
                .par_bridge()
                .map(|input| {
                    log::trace!("process task: {:?}...", &input);
                    let start = Instant::now();
                    let output = process_func(&input);
                    let duration = start.elapsed();
                    log::trace!("process task: {:?}...done", &input);
                    (input, output, Some(duration))
                })
                .collect()
        })
    }
    pub fn parallel_process_with_cost_estimator<I, O, G, F, E>(
        task_generator: G,
        process_func: F,
        cost_estimator: E,
    ) -> Vec<(I, O, Option<Duration>)>
    where
        I: Send + Debug,
        O: Send,
        G: Iterator<Item = I> + ParallelBridge + Send,
        F: Fn(&I) -> O + Sync + Send,
        E: Fn(&I) -> usize + Sync + Send,
    {
        let pool = ThreadPoolBuilder::new()
            .num_threads(crate::config::get_config().threads)
            .build()
            .expect("Failed to build thread pool");

        log::trace!("sorting tasks for load balance...");
        let mut tasks = task_generator.collect::<Vec<_>>();
        tasks.sort_by_cached_key(|ixz| std::cmp::Reverse(cost_estimator(ixz)));
        log::trace!("sorting tasks for load balance...done");
        log::trace!("first 10 items: {:?}", &tasks[..10]);

        pool.install(|| {
            tasks
                .into_iter()
                .par_bridge()
                .map(|input| {
                    log::trace!("process task: {:?}...", &input);
                    let start = Instant::now();
                    let output = process_func(&input);
                    let duration = start.elapsed();
                    log::trace!("process task: {:?}...done", &input);
                    (input, output, Some(duration))
                })
                .collect()
        })
    }
}
pub mod test {
    use std::{fs, path::PathBuf};

    use rand::prelude::*;

    use super::create_chunk_ixz_iter;
    use crate::compress::CompressionType;
    use crate::{
        FileType,
        mca::{ChunkNbt, ChunkWithTimestamp, MCAReader},
        util,
    };

    fn file_type_to_path(file_type: FileType) -> PathBuf {
        let mut path = PathBuf::from("resources/test-payload");
        path.push(PathBuf::from(match file_type {
            FileType::RegionMca => "region/mca",
            FileType::RegionMcc => todo!(),
        }));
        path
    }
    pub fn all_file_iter(
        file_type: FileType,
    ) -> impl Iterator<Item = impl Iterator<Item = PathBuf>> {
        let path = file_type_to_path(file_type);
        fs::read_dir(path).unwrap().filter_map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                Some(fs::read_dir(path).unwrap().filter_map(|entry| {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_file() { Some(path) } else { None }
                }))
            } else {
                None
            }
        })
    }
    pub fn rearranged_nbt(bytes: &Vec<u8>) -> Result<Vec<u8>, fastnbt::error::Error> {
        let de: fastnbt::Value = fastnbt::from_bytes(&bytes)?;
        let sorted = fastnbt::to_bytes(&de)?;
        Ok(sorted)
    }
    pub fn create_test_bytes(seed: u64) -> impl Iterator<Item = Vec<u8>> {
        use rand::prelude::*;
        let mut rng = StdRng::seed_from_u64(seed);

        std::iter::repeat_with(move || {
            let len = rng.random_range(0..10);
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                bytes.push(rng.random_range(0..3) as u8);
            }
            bytes
        })
    }
    pub fn assert_mca_eq(a: &[u8], b: &[u8]) {
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
                assert_eq!(nbt_a, nbt_b);
            } else {
                assert_eq!(chunk_a, chunk_b);
            }
        }
    }
    pub fn assert_mcc_eq(a: Vec<u8>, b: Vec<u8>) {
        let decompressed_a = CompressionType::Zlib.decompress_all(&a).unwrap();
        let nbt_a = util::nbt_serde::de(&decompressed_a);
        let decompressed_b = CompressionType::Zlib.decompress_all(&b).unwrap();
        let nbt_b = util::nbt_serde::de(&decompressed_b);
        assert_eq!(nbt_a, nbt_b);
    }
    pub fn get_test_chunk(path: &PathBuf, rng: &mut StdRng) -> impl Iterator<Item = Vec<u8>> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        let mut xzs = [(0, 0); 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            xzs[i] = (x, z);
        }
        xzs.shuffle(rng);
        xzs.into_iter().map(move |(x, z)| {
            match &reader.get_chunk(x, z).unwrap().unwrap().nbt {
                ChunkNbt::Large => panic!(concat!(
                    "This chunk is too large to save in .mca file, so it do not contains any bytes. ",
                    "If you are testing, use another .mca file instead.",
                )),
                ChunkNbt::Small(nbt) => nbt.clone(),
            }
        })
    }
    pub fn get_test_chunk_by_xz(path: &PathBuf, x: usize, z: usize) -> Option<ChunkWithTimestamp> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        reader.get_chunk(x, z).unwrap().cloned()
    }
}
