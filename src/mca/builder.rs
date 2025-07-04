use super::{ChunkWithTimestamp, MCAError, SECTOR_SIZE};
use crate::{
    compress::CompressionType,
    mca::{ChunkNbt, LARGE_FLAG},
    util::{create_chunk_ixz_iter, parallel::parallel_process_with_cost_estimator},
};

pub struct MCABuilder<'a> {
    chunks: [Option<&'a ChunkWithTimestamp>; 1024],
}
impl<'a> MCABuilder<'a> {
    pub fn new() -> Self {
        Self {
            chunks: [None; 1024],
        }
    }
    pub fn set_chunk(&mut self, x: usize, z: usize, chunk: &'a ChunkWithTimestamp) {
        let i = x + z * 32;
        self.chunks[i] = Some(chunk);
    }
    pub fn to_bytes(&self, compression_type: CompressionType) -> Result<Vec<u8>, MCAError> {
        // parallel compression
        let mut results = parallel_process_with_cost_estimator(
            create_chunk_ixz_iter(),
            |(i, x, z)| match self.chunks[*i] {
                None => None,
                Some(chunk) => match &chunk.nbt {
                    ChunkNbt::Large => None,
                    ChunkNbt::Small(nbt) => Some(compression_type.compress_all(nbt).map_err(|e| {
                        MCAError::Compression {
                            x: *x,
                            z: *z,
                            reason: e.to_string(),
                        }
                    })),
                },
            },
            |(i, _, _)| match self.chunks[*i] {
                None => 0,
                Some(chunk) => match &chunk.nbt {
                    ChunkNbt::Large => 0,
                    ChunkNbt::Small(nbt) => nbt.len(),
                },
            },
        );
        results.sort_by_key(|(ixz, ..)| ixz.0);

        let header_size = SECTOR_SIZE * 2;
        let chunks_count = self.chunks.iter().filter(|e| e.is_some()).count();
        let chunk_estimated_size = match compression_type {
            CompressionType::No => 0x40000, // 128KB
            _ => 0x8000,                    // 16KB
        };
        let mut buffer: Vec<u8> =
            Vec::with_capacity(header_size + chunk_estimated_size * chunks_count);

        // prefill header
        buffer.extend_from_slice(&[0; SECTOR_SIZE * 2]);

        for ((i, _, _), compressed_nbt, _) in results {
            let nbt = match compressed_nbt {
                Some(Ok(nbt)) => Some(nbt),
                Some(Err(e)) => return Err(e),
                None => None,
            };

            let chunk = self.chunks[i];

            // calculate header info
            let (sector_offset, sector_count, timestamp) = match chunk {
                None => (0, 0, 0),
                Some(chunk) => {
                    let sector_offset = buffer.len() / SECTOR_SIZE;
                    match nbt {
                        Some(ref nbt) => {
                            // `+ 5` for chunk data header (4 for length and 1 for compression type)
                            // `+ SECTOR_SIZE - 1` for align to SECTOR_SIZE
                            let sector_count = (nbt.len() + 5 + SECTOR_SIZE - 1) / SECTOR_SIZE;
                            (sector_offset, sector_count, chunk.timestamp)
                        }
                        None => (sector_offset, 1, chunk.timestamp),
                    }
                }
            };

            // write body if chunk exists
            if let Some(_) = chunk {
                // small chunk
                if let Some(nbt) = nbt {
                    buffer.extend_from_slice(&(nbt.len() as u32 + 1).to_be_bytes());
                    buffer.push(compression_type.to_magic());
                    buffer.extend_from_slice(&nbt);
                    let padding_size = sector_count * SECTOR_SIZE - (nbt.len() + 5);
                    buffer.extend(std::iter::repeat(0).take(padding_size));
                }
                // large chunk
                else {
                    buffer.extend_from_slice(&1u32.to_be_bytes());
                    buffer.push((compression_type.to_magic()) | LARGE_FLAG);
                    let padding_size = sector_count * SECTOR_SIZE - 5;
                    buffer.extend(std::iter::repeat(0).take(padding_size));
                }
            }

            // update header: location part
            let header_loc_offset = i * 4;
            buffer[header_loc_offset..header_loc_offset + 3]
                .copy_from_slice(&(sector_offset as u32).to_be_bytes()[1..4]);
            buffer[header_loc_offset + 3] = sector_count as u8;

            // update header: timestamp part
            let header_ts_offset = header_loc_offset + SECTOR_SIZE;
            buffer[header_ts_offset..header_ts_offset + 4]
                .copy_from_slice(&timestamp.to_be_bytes());
        }
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        config::{Config, with_test_config},
        mca::{LazyChunk, MCAReader},
    };

    use super::*;

    static TEST_CONFIG: Config = Config {
        log_config: crate::config::LogConfig::Trace,
        threads: 16,
    };

    #[test]
    fn test_to_bytes() {
        with_test_config(TEST_CONFIG.clone(), || {
            let mca_0 =
                fs::read("./resources/test-payload/region/mca/hairlessvillager-0/20250516.mca")
                    .expect("Failed to read test MCA file");

            let reader_0 = MCAReader::from_bytes(&mca_0).expect("Failed to create MCA reader");
            let mut builder_0 = MCABuilder::new();
            for (_, x, z) in create_chunk_ixz_iter() {
                let chunk = reader_0.get_chunk_lazily(x, z);
                match chunk {
                    LazyChunk::Unloaded => panic!("Invalid MCAReader"),
                    LazyChunk::NotExists => (),
                    LazyChunk::Some(chunk) => builder_0.set_chunk(x, z, &chunk),
                }
            }
            let mca_1 = builder_0
                .to_bytes(CompressionType::Zlib)
                .expect("Failed to build MCA bytes");

            let reader_1 = MCAReader::from_bytes(&mca_1)
                .expect("Failed to create MCA reader from built bytes");
            let mut builder_1 = MCABuilder::new();
            for (_, x, z) in create_chunk_ixz_iter() {
                let chunk = reader_1.get_chunk_lazily(x, z);
                match chunk {
                    LazyChunk::Unloaded => panic!("Invalid MCAReader"),
                    LazyChunk::NotExists => (),
                    LazyChunk::Some(chunk) => builder_1.set_chunk(x, z, &chunk),
                }
            }
            let mca_2 = builder_1
                .to_bytes(CompressionType::Zlib)
                .expect("Failed to rebuild MCA bytes");

            assert_eq!(mca_1, mca_2);
        });
    }
}
