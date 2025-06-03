use std::collections::{BTreeMap, BTreeSet};

pub fn create_chunk_ixz_iter() -> impl Iterator<Item = (usize, usize, usize)> {
    (0..32).flat_map(|z| {
        (0..32).map(move |x| {
            let i = x + 32 * z;
            (i, x, z)
        })
    })
}

pub fn wrap_with_root_compound(value: fastnbt::Value) -> fastnbt::Value {
    fastnbt::Value::Compound(std::collections::BTreeMap::from([(
        "root".to_string(),
        value,
    )]))
}
pub fn unwrap_with_root_compound(value: fastnbt::Value) -> fastnbt::Value {
    match value {
        fastnbt::Value::Compound(mut map) => map.remove("root").unwrap(),
        _ => panic!("root compound not exists"),
    }
}

pub fn fastnbt_serialize(v: &fastnbt::Value) -> Vec<u8> {
    fastnbt::to_bytes(v).unwrap()
}
pub fn fastnbt_deserialize(input: &[u8]) -> fastnbt::Value {
    fastnbt::from_bytes(input).unwrap()
}

pub fn merge_map<K, V>(
    mut map1: BTreeMap<K, V>,
    mut map2: BTreeMap<K, V>,
) -> BTreeMap<K, (Option<V>, Option<V>)>
where
    K: Ord + Clone,
{
    let all_keys = BTreeSet::from_iter(map1.keys().chain(map2.keys()).map(|k| k.clone()));
    BTreeMap::from_iter(all_keys.into_iter().map(|key| {
        let e1 = map1.remove(&key);
        let e2 = map2.remove(&key);
        (key, (e1, e2))
    }))
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
    use fastnbt::Value;
    use rand::prelude::*;

    use crate::mca::{ChunkWithTimestamp, LazyChunk, MCABuilder, MCAReader};

    use super::{compress::CompressionType, create_chunk_ixz_iter};

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
    pub fn build_test_mca(path: &str, chunks: usize, rng: &mut StdRng) -> Vec<u8> {
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
    pub fn build_test_mca_with_one_chunk(path: &str, x: usize, z: usize) -> Vec<u8> {
        let reader = MCAReader::from_file(path, false).unwrap();
        let mut builder = MCABuilder::new();
        let chunk = reader.get_chunk_lazily(x, z);
        if let LazyChunk::Some(chunk) = chunk {
            builder.set_chunk(x, z, chunk);
        } else {
            panic!("chunk is not avaliable");
        }
        builder.to_bytes(CompressionType::Zlib)
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
    pub fn get_test_chunk(path: &str, rng: &mut StdRng) -> impl Iterator<Item = Vec<u8>> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        let mut xzs = [(0, 0); 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            xzs[i] = (x, z);
        }
        xzs.shuffle(rng);
        xzs.into_iter()
            .map(move |(x, z)| reader.get_chunk(x, z).unwrap().unwrap().nbt.clone())
    }
    pub fn get_test_chunk_by_xz(path: &str, x: usize, z: usize) -> ChunkWithTimestamp {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        reader.get_chunk(x, z).unwrap().unwrap().clone()
    }
}

pub mod compress {
    use std::io::{Read, Write};

    #[derive(Debug, Clone, Copy)]
    pub enum CompressionType {
        GZip,
        Zlib,
        NoCompression,
        LZ4,
    }
    impl CompressionType {
        pub fn to_magic(&self) -> u8 {
            match self {
                CompressionType::GZip => 1,
                CompressionType::Zlib => 2,
                CompressionType::NoCompression => 3,
                CompressionType::LZ4 => 4,
            }
        }
        pub fn from_magic(magic: u8) -> Self {
            match magic {
                1 => CompressionType::GZip,
                2 => CompressionType::Zlib,
                3 => CompressionType::NoCompression,
                4 => CompressionType::LZ4,
                _ => panic!("unsupported compression type/magic"),
            }
        }
        pub fn compress(&self, data: &Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            match self {
                CompressionType::GZip => {
                    let mut encoder =
                        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                    encoder
                        .write_all(data)
                        .map_err(|e| format!("failed to write data to GzEncoder: {}", &e))?;
                    Ok(encoder.finish().map_err(|e| {
                        format!("failed to finish compression of GzEncoder: {}", &e)
                    })?)
                }
                CompressionType::Zlib => {
                    let mut encoder =
                        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    encoder
                        .write_all(data)
                        .map_err(|e| format!("failed to write data to ZlibEncoder: {}", &e))?;
                    Ok(encoder.finish().map_err(|e| {
                        format!("failed to finish compression of ZlibEncoder: {}", &e)
                    })?)
                }
                CompressionType::NoCompression => Ok(data.to_vec()),
                CompressionType::LZ4 => {
                    let compressed = lz4_flex::block::compress_prepend_size(data);
                    Ok(compressed)
                }
            }
        }
        pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            match self {
                CompressionType::GZip => {
                    let mut decoder = flate2::read::GzDecoder::new(data);
                    let mut decompressed = Vec::new();
                    decoder
                        .read_to_end(&mut decompressed)
                        .map_err(|e| format!("failed to decompress with GzDecoder: {}", &e))?;
                    Ok(decompressed)
                }
                CompressionType::Zlib => {
                    let mut decoder = flate2::read::ZlibDecoder::new(data);
                    let mut decompressed = Vec::new();
                    decoder
                        .read_to_end(&mut decompressed)
                        .map_err(|e| format!("failed to decompress with ZlibEncoder: {}", &e))?;
                    Ok(decompressed)
                }
                CompressionType::NoCompression => Ok(data.to_vec()),
                CompressionType::LZ4 => {
                    let mut decompressed = Vec::new();
                    lz4_flex::block::decompress_into(data, &mut decompressed)
                        .map_err(|e| format!("failed to decompress with LZ4: {}", &e))?;
                    Ok(decompressed)
                }
            }
        }
    }
}
