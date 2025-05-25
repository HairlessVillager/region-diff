use super::{ChunkWithTimestamp, SECTOR_SIZE};
use crate::util::{compress::CompressionType, create_chunk_ixz_iter};

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
    pub fn to_bytes(&self, compression_type: CompressionType) -> Vec<u8> {
        let header_size = SECTOR_SIZE * 2;
        let chunks_count = self.chunks.iter().filter(|e| e.is_some()).count();
        let chunk_estimated_size = match compression_type {
            CompressionType::NoCompression => 0x40000, // 128KB
            _ => 0x8000,                               // 16KB
        };
        let mut buffer: Vec<u8> =
            Vec::with_capacity(header_size + chunk_estimated_size * chunks_count);

        // prefill header
        buffer.extend_from_slice(&[0; SECTOR_SIZE * 2]);

        for (i, _, _) in create_chunk_ixz_iter() {
            let chunk = self.chunks[i];
            assert!(buffer.len() % SECTOR_SIZE == 0);
            let (sector_offset, sector_count, timestamp, compressed_nbt): (
                usize,
                usize,
                u32,
                Option<Vec<u8>>,
            ) = match chunk {
                None => (0, 0, 0, None),
                Some(chunk) => {
                    let sector_offset = buffer.len() / SECTOR_SIZE;
                    let compressed_nbt = compression_type.compress(&chunk.nbt).unwrap();

                    // `+ 5` for chunk data header (4 for length and 1 for compression type)
                    // `+ SECTOR_SIZE - 1` for align to SECTOR_SIZE
                    let sector_count = (compressed_nbt.len() + 5 + SECTOR_SIZE - 1) / SECTOR_SIZE;

                    let timestamp = chunk.timestamp;
                    (sector_offset, sector_count, timestamp, Some(compressed_nbt))
                }
            };

            // fill chunk data
            if let Some(nbt) = compressed_nbt {
                buffer.extend_from_slice(&((nbt.len() + 1) as u32).to_be_bytes()[0..4]); // `+ 1` for compression type
                buffer.extend_from_slice(&(compression_type.to_magic() as u8).to_be_bytes()[0..1]);
                buffer.extend_from_slice(&nbt);
                let padding_size = sector_count * SECTOR_SIZE - (nbt.len() + 5); // `+ 5` for chunk data header
                buffer.extend(std::iter::repeat(0).take(padding_size));
            }

            // fill header's location part
            let header_loc_offset = i * 4;
            buffer[header_loc_offset..header_loc_offset + 3]
                .copy_from_slice(&(sector_offset as u32).to_be_bytes()[1..4]);
            buffer[header_loc_offset + 3..header_loc_offset + 4]
                .copy_from_slice(&(sector_count as u8).to_be_bytes()[0..1]);

            // fill header's timestamp part
            let header_ts_offset = header_loc_offset + SECTOR_SIZE;
            buffer[header_ts_offset..header_ts_offset + 4]
                .copy_from_slice(&(timestamp as u32).to_be_bytes()[0..4]);
        }
        buffer
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::mca::{LazyChunk, MCAReader};

    use super::*;
    #[test]
    fn test_to_bytes() {
        let mca_0 = fs::read("./resources/mca/r.1.2.20250516.mca").unwrap();

        let reader_0 = MCAReader::from_bytes(&mca_0).unwrap();
        let mut builder_0 = MCABuilder::new();
        for (_, x, z) in create_chunk_ixz_iter() {
            let chunk = reader_0.get_chunk_lazily(x, z);
            match chunk {
                LazyChunk::Unloaded => panic!("invalid MCAReader"),
                LazyChunk::NotExists => (),
                LazyChunk::Some(chunk) => builder_0.set_chunk(x, z, chunk),
            }
        }
        let mca_1 = builder_0.to_bytes(CompressionType::Zlib);

        let reader_1 = MCAReader::from_bytes(&mca_1).unwrap();
        let mut builder_1 = MCABuilder::new();
        for (_, x, z) in create_chunk_ixz_iter() {
            let chunk = reader_1.get_chunk_lazily(x, z);
            match chunk {
                LazyChunk::Unloaded => panic!("invalid MCAReader"),
                LazyChunk::NotExists => (),
                LazyChunk::Some(chunk) => builder_1.set_chunk(x, z, chunk),
            }
        }
        let mca_2 = builder_1.to_bytes(CompressionType::Zlib);

        assert_eq!(mca_1, mca_2);
    }
}
