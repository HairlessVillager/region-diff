pub type IXZ = (usize, usize, usize);
pub fn create_chunk_ixz_iter() -> impl Iterator<Item = IXZ> {
    (0..32).flat_map(|z| {
        (0..32).map(move |x| {
            let i = x + 32 * z;
            (i, x, z)
        })
    })
}

pub fn fastnbt_serialize(v: &fastnbt::Value) -> Vec<u8> {
    fastnbt::to_bytes(v).unwrap()
}
pub fn fastnbt_deserialize(input: &[u8]) -> fastnbt::Value {
    fastnbt::from_bytes(input).unwrap()
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

    pub fn serialize<T: Encode>(val: T) -> Vec<u8> {
        encode_to_vec(val, CONFIG.clone()).unwrap()
    }
    pub fn deserialize<T: Decode<()>>(data: &Vec<u8>) -> T {
        decode_from_slice(data, CONFIG.clone())
            .map(|(de, _)| de)
            .unwrap()
    }
}

#[cfg(test)]
pub mod test {
    use std::{fs, path::PathBuf};

    use fastnbt::Value;
    use rand::prelude::*;

    use crate::{
        FileType,
        mca::{ChunkWithTimestamp, MCAReader},
    };

    use super::create_chunk_ixz_iter;

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
    #[allow(dead_code)]
    pub fn file_iter(file_type: FileType, name: String) -> impl Iterator<Item = PathBuf> {
        let mut path = file_type_to_path(file_type);
        path.push(PathBuf::from(name));
        fs::read_dir(path).unwrap().filter_map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() { Some(path) } else { None }
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
                assert_eq!(
                    fastnbt::from_bytes::<Value>(nbt_a),
                    fastnbt::from_bytes::<Value>(nbt_b)
                );
            } else {
                assert_eq!(chunk_a, chunk_b);
            }
        }
    }
    pub fn get_test_chunk(path: &PathBuf, rng: &mut StdRng) -> impl Iterator<Item = Vec<u8>> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        let mut xzs = [(0, 0); 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            xzs[i] = (x, z);
        }
        xzs.shuffle(rng);
        xzs.into_iter()
            .map(move |(x, z)| reader.get_chunk(x, z).unwrap().unwrap().nbt.clone())
    }
    pub fn get_test_chunk_by_xz(path: &PathBuf, x: usize, z: usize) -> Option<ChunkWithTimestamp> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        reader.get_chunk(x, z).unwrap().cloned()
    }
}
