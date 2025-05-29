use std::collections::{BTreeMap, BTreeSet};

use crate::{err::Error, object::Object, storage::StorageBackend};

pub fn create_chunk_ixz_iter() -> impl Iterator<Item = (usize, usize, usize)> {
    (0..32).flat_map(|z| {
        (0..32).map(move |x| {
            let i = x + 32 * z;
            (i, x, z)
        })
    })
}

pub fn create_bincode_config() -> bincode::config::Configuration<bincode::config::BigEndian> {
    bincode::config::standard()
        .with_big_endian()
        .with_variable_int_encoding()
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

pub fn put_object<S: StorageBackend, O: Object>(backend: &mut S, obj: &O) -> Result<(), Error> {
    let (key, value) = obj.as_kv();
    backend.put(key, value)
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

    use bincode::{decode_from_slice, encode_to_vec};

    use crate::err::Error;

    use super::create_bincode_config;

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
        pub fn compress(&self, data: &Vec<u8>) -> Result<Vec<u8>, Error> {
            match self {
                CompressionType::GZip => {
                    let mut encoder =
                        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                    encoder.write_all(data).map_err(|e| {
                        Error::from_msg_err("failed to write data to GzEncoder", &e)
                    })?;
                    Ok(encoder.finish().map_err(|e| {
                        Error::from_msg_err("failed to finish compression of GzEncoder", &e)
                    })?)
                }
                CompressionType::Zlib => {
                    let mut encoder =
                        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    encoder.write_all(data).map_err(|e| {
                        Error::from_msg_err("failed to write data to ZlibEncoder", &e)
                    })?;
                    Ok(encoder.finish().map_err(|e| {
                        Error::from_msg_err("failed to finish compression of ZlibEncoder", &e)
                    })?)
                }
                CompressionType::NoCompression => Ok(data.to_vec()),
                CompressionType::LZ4 => {
                    let compressed = lz4_flex::block::compress_prepend_size(data);
                    Ok(compressed)
                }
            }
        }
        pub fn compress_with_type(&self, data: &Vec<u8>) -> Result<Vec<u8>, Error> {
            let data = (self.to_magic(), data);
            let data = encode_to_vec(data, create_bincode_config())
                .map_err(|e| Error::from_msg_err("failed to serialize", &e))?;
            self.compress(&data)
        }
        pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            match self {
                CompressionType::GZip => {
                    let mut decoder = flate2::read::GzDecoder::new(data);
                    let mut decompressed = Vec::new();
                    decoder.read_to_end(&mut decompressed).map_err(|e| {
                        Error::from_msg_err("failed to decompress with GzDecoder", &e)
                    })?;
                    Ok(decompressed)
                }
                CompressionType::Zlib => {
                    let mut decoder = flate2::read::ZlibDecoder::new(data);
                    let mut decompressed = Vec::new();
                    decoder.read_to_end(&mut decompressed).map_err(|e| {
                        Error::from_msg_err("failed to decompress with ZlibEncoder", &e)
                    })?;
                    Ok(decompressed)
                }
                CompressionType::NoCompression => Ok(data.to_vec()),
                CompressionType::LZ4 => {
                    let mut decompressed = Vec::new();
                    lz4_flex::block::decompress_into(data, &mut decompressed)
                        .map_err(|e| Error::from_msg_err("failed to decompress with LZ4", &e))?;
                    Ok(decompressed)
                }
            }
        }
        pub fn decompress_with_type(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            let data = self.decompress(&data)?;
            let data: ((u8, Vec<u8>), usize) = decode_from_slice(&data, create_bincode_config())
                .map_err(|e| Error::from_msg_err("failed to deserialize", &e))?;
            let ((magic, data), _) = data;
            Self::from_magic(magic).decompress(&data)
        }
    }
}
