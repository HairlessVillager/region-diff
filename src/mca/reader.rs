use std::io::{Cursor, Read, Seek};
use std::path::PathBuf;

use crate::compress::CompressionType;
use crate::util::{create_chunk_ixz_iter, parallel::parallel_process_with_cost_estimator};

use super::{ChunkNbt, ChunkWithTimestamp, HeaderEntry, LARGE_FLAG, MCAError, SECTOR_SIZE};

#[derive(Debug, Clone)]
pub enum LazyChunk {
    Unloaded,
    NotExists,
    Some(ChunkWithTimestamp),
}
pub struct MCAReader<R: Read + Seek> {
    #[allow(dead_code)]
    mca_reader: R,
    header: [HeaderEntry; 1024],
    chunks: [LazyChunk; 1024],
}

impl<R: Read + Seek> MCAReader<R> {
    fn from_reader(mut reader: R, lazy: bool) -> Result<Self, MCAError> {
        let mut chunks = [const { LazyChunk::Unloaded }; 1024];
        let header = read_header(&mut reader)?;

        if !lazy {
            let mut header_refs: Vec<&HeaderEntry> = header.iter().collect();
            header_refs.sort_by_key(|e| e.sector_offset);
            for header_entry in header_refs {
                chunks[header_entry.idx] = match header_entry.sector_offset {
                    0 => LazyChunk::NotExists,
                    1..=u32::MAX => {
                        let offset = (header_entry.sector_offset as u64) * (SECTOR_SIZE as u64);
                        reader.seek(std::io::SeekFrom::Start(offset))?;

                        let mut sector_buf =
                            vec![0u8; header_entry.sector_count as usize * SECTOR_SIZE];
                        reader.read_exact(&mut sector_buf)?;
                        LazyChunk::Some(ChunkWithTimestamp {
                            timestamp: header_entry.timestamp,
                            nbt: read_chunk_nbt(
                                &sector_buf,
                                header_entry.idx % 32,
                                header_entry.idx / 32,
                            )?,
                        })
                    }
                }
            }
        }
        Ok(Self {
            mca_reader: reader,
            header,
            chunks,
        })
    }
    #[allow(dead_code)]
    pub fn get_chunk(
        &mut self,
        x: usize,
        z: usize,
    ) -> Result<Option<&ChunkWithTimestamp>, MCAError> {
        use std::io::SeekFrom;

        let idx = x + 32 * z;

        if let LazyChunk::Some(ref chunk) = self.chunks[idx] {
            return Ok(Some(chunk));
        }
        if let LazyChunk::NotExists = self.chunks[idx] {
            return Ok(None);
        }

        let header = &self.header[idx];
        if !header.is_available()? {
            return Ok(None);
        }

        let mut sector_buf = vec![0u8; header.sector_count as usize * SECTOR_SIZE];
        let offset = (header.sector_offset as usize) * SECTOR_SIZE;
        self.mca_reader.seek(SeekFrom::Start(offset as u64))?;
        self.mca_reader.read_exact(&mut sector_buf)?;

        let chunk = ChunkWithTimestamp {
            timestamp: header.timestamp,
            nbt: read_chunk_nbt(&sector_buf, x, z)?,
        };

        self.chunks[idx] = LazyChunk::Some(chunk);

        match &self.chunks[idx] {
            LazyChunk::Some(chunk) => Ok(Some(chunk)),
            _ => Err(MCAError::ChunkLoadFailed {
                x,
                z,
                reason: "Failed to load chunk data".to_string(),
            }),
        }
    }
    pub fn get_chunk_lazily(&self, x: usize, z: usize) -> &LazyChunk {
        let idx = x + 32 * z;
        &self.chunks[idx]
    }
    pub fn get_timestamp(&self, x: usize, z: usize) -> u32 {
        let idx = x + 32 * z;
        self.header[idx].timestamp
    }
}

impl MCAReader<std::io::BufReader<std::fs::File>> {
    pub fn from_file(path: &PathBuf, lazy: bool) -> Result<Self, MCAError> {
        use std::{fs::File, io::BufReader};
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_reader(reader, lazy)
    }
}
impl<'a> MCAReader<Cursor<&'a [u8]>> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, MCAError> {
        let mut chunks = [const { LazyChunk::Unloaded }; 1024];
        let mut reader = Cursor::new(bytes);
        let header = read_header(&mut reader)?;

        let results = parallel_process_with_cost_estimator(
            create_chunk_ixz_iter(),
            |(i, x, z)| {
                let header_entry = &header[*i];
                match header_entry.sector_offset {
                    0 => Ok(None),
                    1..=u32::MAX => {
                        let offset = header_entry.sector_offset as usize * SECTOR_SIZE;
                        let size = header_entry.sector_count as usize * SECTOR_SIZE;
                        let sector_data = &bytes[offset..offset + size];
                        Ok(Some(ChunkWithTimestamp {
                            timestamp: header_entry.timestamp,
                            nbt: read_chunk_nbt(&sector_data, *x, *z)?,
                        }))
                    }
                }
            },
            |(i, _, _)| header[*i].sector_count as usize,
        );

        for ((i, _, _), chunk_result, _) in results {
            chunks[i] = match chunk_result {
                Ok(Some(chunk)) => LazyChunk::Some(chunk),
                Ok(None) => LazyChunk::NotExists,
                Err(e) => return Err(e),
            };
        }

        Ok(Self {
            mca_reader: reader,
            header,
            chunks,
        })
    }
}
fn read_header<R: Read + Seek>(reader: &mut R) -> Result<[HeaderEntry; 1024], MCAError> {
    let mut headers = std::array::from_fn(|_| HeaderEntry {
        idx: 0,
        sector_offset: 0,
        sector_count: 0,
        timestamp: 0,
    });

    // read locations
    for (idx, _offset) in (0x0000..0x0fff).step_by(4).enumerate() {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let sector_offset = u32::from_be_bytes([0, buf[0], buf[1], buf[2]]);
        let sector_count = buf[3];
        headers[idx] = HeaderEntry {
            idx,
            sector_offset,
            sector_count,
            timestamp: 0,
        };
    }

    // read timestamps
    for (idx, _offset) in (0x1000..0x1fff).step_by(4).enumerate() {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let timestamp = u32::from_be_bytes(buf);
        headers[idx].timestamp = timestamp;
    }

    Ok(headers)
}

fn read_chunk_nbt(sector_buf: &[u8], x: usize, z: usize) -> Result<ChunkNbt, MCAError> {
    let length =
        u32::from_be_bytes([sector_buf[0], sector_buf[1], sector_buf[2], sector_buf[3]]) as usize;

    let compression_type = sector_buf[4];
    let data = &sector_buf[5..length + 4];

    match compression_type & LARGE_FLAG {
        LARGE_FLAG => Ok(ChunkNbt::Large),
        _ => {
            let nbt = CompressionType::from_magic(compression_type)
                .decompress_all(data)
                .map_err(|e| MCAError::Compression {
                    x,
                    z,
                    reason: e.to_string(),
                })?;
            Ok(ChunkNbt::Small(nbt))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Config, with_test_config},
        util::{create_chunk_ixz_iter, test::all_file_iter},
    };
    use std::io::Write;
    static TEST_CONFIG: Config = Config {
        log_config: crate::config::LogConfig::NoLog,
        threads: 16,
    };

    fn create_test_mca() -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut file = Cursor::new(&mut buffer);

        let mut header = vec![0u8; SECTOR_SIZE * 2];

        // set header for first chunk
        header[0] = 0;
        header[1] = 0;
        header[2] = 2; // sector offset
        header[3] = 1; // sector count
        header[4096] = 0;
        header[4097] = 0;
        header[4098] = 0;
        header[4099] = 1; // timestamp = 1

        file.write_all(&header).expect("Failed to write header");

        // create chunk data for first chunk (using zlib compression)
        let chunk_data = vec![1u8; 100]; // example NBT data
        let mut compressed = Vec::new();
        {
            let mut encoder =
                flate2::write::ZlibEncoder::new(&mut compressed, flate2::Compression::default());
            encoder
                .write_all(&chunk_data)
                .expect("Failed to write chunk data");
            encoder.finish().expect("Failed to finish compression");
        }

        file.write_all(&((compressed.len() + 1) as u32).to_be_bytes())
            .expect("Failed to write chunk length"); // write chunk length, +1 for compression type byte
        file.write_all(&[2])
            .expect("Failed to write compression type"); // write compression type (2 = zlib)
        file.write_all(&compressed)
            .expect("Failed to write compressed data"); // write compressed data

        // padding to 4096 bytes (one sector)
        let padding = vec![0u8; SECTOR_SIZE - (compressed.len() + 4)];
        file.write_all(&padding).expect("Failed to write padding");

        buffer
    }

    #[test]
    fn test_header_reading() {
        let mut mca = create_test_mca();
        let mut reader = Cursor::new(&mut mca);
        let headers = read_header(&mut reader).expect("Failed to read header");

        // test header for first chunk
        let header_entry = &headers[0];
        assert_eq!(header_entry.sector_offset, 2);
        assert_eq!(header_entry.sector_count, 1);
        assert_eq!(header_entry.timestamp, 1);

        // test header for second chunk should be empty
        let header_entry = &headers[1];
        assert!(
            !header_entry
                .is_available()
                .expect("Failed to check availability")
        );
    }

    #[test]
    fn test_mca_file_reading() {
        with_test_config(TEST_CONFIG.clone(), || {
            let mut mca = create_test_mca();
            let mca = MCAReader::from_bytes(&mut mca).expect("Failed to create MCA reader");

            // test first chunk
            let chunk = mca.chunks[0].clone();
            match chunk {
                LazyChunk::Some(chunk) => {
                    assert_eq!(chunk.timestamp, 1);
                    match chunk.nbt {
                        ChunkNbt::Large => panic!("Chunk should not so large"),
                        ChunkNbt::Small(nbt) => assert!(!nbt.is_empty()),
                    }
                }
                _ => panic!("Chunk should be Some, but got {:?}", chunk),
            }
            // test second chunk should be empty
            let chunk = mca.chunks[1].clone();
            match chunk {
                LazyChunk::NotExists => (),
                _ => panic!("Chunk should be NotExists, but got {:?}", chunk),
            }
        });
    }

    #[test]
    fn test_real_files_reading() {
        for paths in all_file_iter(crate::FileType::RegionMca) {
            for path in paths {
                let mut reader =
                    MCAReader::from_file(&path, false).expect("Failed to read MCA file");
                for (_, x, z) in create_chunk_ixz_iter() {
                    let _ = reader.get_chunk(x, z).expect("Failed to get chunk");
                }
            }
        }
    }
}
