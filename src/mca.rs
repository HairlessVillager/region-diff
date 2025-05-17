use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};

#[derive(Debug, Clone)]
struct HeaderEntry {
    idx: usize,
    sector_offset: u64,
    sector_count: u64,
    timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum LazyChunk {
    Unloaded,
    NotExists,
    Some(ChunkWithTimestamp),
}
#[derive(Debug, Clone)]
pub struct ChunkWithTimestamp {
    pub timestamp: i64,
    pub nbt: Vec<u8>,
}

pub struct MCAReader {
    mca_reader: BufReader<File>,
    header: [Option<HeaderEntry>; 1024],
    chunks: [LazyChunk; 1024],
}

impl MCAReader {
    pub fn from_file(file_path: &str, lazy: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(file_path)?;
        let mut reader = BufReader::new(file);
        let mut chunks = [const { LazyChunk::Unloaded }; 1024];
        let header = read_header(&mut reader)?;

        if !lazy {
            let mut filtered_header: Vec<HeaderEntry> =
                header.iter().filter_map(|h| h.as_ref()).cloned().collect();
            filtered_header.sort_by_key(|e| e.sector_offset);
            for header_entry in filtered_header {
                let mut sector_buf = vec![0u8; (header_entry.sector_count * 4096) as usize];
                reader.read_exact(&mut sector_buf)?;
                chunks[header_entry.idx] = LazyChunk::Some(ChunkWithTimestamp {
                    timestamp: header_entry.timestamp as i64,
                    nbt: read_chunk_nbt(&sector_buf)?,
                });
            }
        }

        Ok(Self {
            mca_reader: reader,
            header,
            chunks,
        })
    }

    pub fn get_chunk(
        &mut self,
        x: i32,
        z: i32,
    ) -> Result<Option<&ChunkWithTimestamp>, Box<dyn std::error::Error>> {
        let idx = (x + 32 * z) as usize;

        if let LazyChunk::Some(ref chunk) = self.chunks[idx] {
            return Ok(Some(chunk));
        }
        if let LazyChunk::NotExists = self.chunks[idx] {
            return Ok(None);
        }

        let header = match self.header[idx].as_ref() {
            Some(h) => h,
            None => {
                self.chunks[idx] = LazyChunk::NotExists;
                return Ok(None);
            }
        };

        let mut sector_buf = vec![0u8; (header.sector_count * 4096) as usize];
        self.mca_reader
            .seek(SeekFrom::Start(header.sector_offset * 4096))?;
        self.mca_reader.read_exact(&mut sector_buf)?;

        let chunk = ChunkWithTimestamp {
            timestamp: header.timestamp as i64,
            nbt: read_chunk_nbt(&sector_buf)?,
        };

        self.chunks[idx] = LazyChunk::Some(chunk);

        match &self.chunks[idx] {
            LazyChunk::Some(chunk) => Ok(Some(chunk)),
            _ => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to load chunk",
            ))),
        }
    }
}

fn read_header(
    reader: &mut BufReader<File>,
) -> Result<[Option<HeaderEntry>; 1024], Box<dyn std::error::Error>> {
    let mut headers = std::array::from_fn(|_| None);

    // read locations
    for (idx, _offset) in (0x0000..0x0fff).step_by(4).enumerate() {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let sector_offset = u32::from_be_bytes([0, buf[0], buf[1], buf[2]]) as u64;
        let sector_count = buf[3] as u64;
        if sector_count > 0 && sector_offset > 0 {
            headers[idx] = Some(HeaderEntry {
                idx,
                sector_offset,
                sector_count,
                timestamp: 0,
            });
        }
    }

    // read timestamps
    for (idx, _offset) in (0x1000..0x1fff).step_by(4).enumerate() {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let timestamp = i32::from_be_bytes(buf) as u64;
        match headers[idx] {
            Some(ref mut header) => header.timestamp = timestamp,
            None => {
                if timestamp > 0 {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Header entry not found",
                    )));
                }
            }
        }
    }

    Ok(headers)
}

fn decompress_nbt(
    data: &[u8],
    compression_type: u8,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match compression_type {
        1 => {
            let mut decoder = flate2::read::GzDecoder::new(data);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        2 => {
            let mut decoder = flate2::read::ZlibDecoder::new(data);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        3 => Ok(data.to_vec()),
        4 => {
            let mut decompressed = Vec::new();
            lz4_flex::block::decompress_into(data, &mut decompressed)?;
            Ok(decompressed)
        }
        _ => Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Unsupported compression type: {}", compression_type),
        ))),
    }
}

fn read_chunk_nbt(sector_buf: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let length =
        u32::from_be_bytes([sector_buf[0], sector_buf[1], sector_buf[2], sector_buf[3]]) as usize;

    let compression_type = sector_buf[4];
    let data = &sector_buf[5..length + 4];

    let nbt = decompress_nbt(data, compression_type)?;
    Ok(nbt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    fn create_test_mca(test_file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if Path::new(test_file_path).exists() {
            return Ok(());
        }

        let mut file = File::create(test_file_path)?;

        let mut header = vec![0u8; 8192];

        // set header for first chunk
        header[0] = 0;
        header[1] = 0;
        header[2] = 2; // sector offset
        header[3] = 1; // sector count
        header[4096] = 0;
        header[4097] = 0;
        header[4098] = 0;
        header[4099] = 1; // timestamp = 1

        file.write_all(&header)?;

        // create chunk data for first chunk (using zlib compression)
        let chunk_data = vec![1u8; 100]; // example NBT data
        let mut compressed = Vec::new();
        {
            let mut encoder =
                flate2::write::ZlibEncoder::new(&mut compressed, flate2::Compression::default());
            encoder.write_all(&chunk_data)?;
            encoder.finish()?;
        }

        file.write_all(&((compressed.len() + 1) as u32).to_be_bytes())?; // write chunk length, +1 for compression type byte
        file.write_all(&[2])?; // write compression type (2 = zlib)
        file.write_all(&compressed)?; // write compressed data

        // padding to 4096 bytes (one sector)
        let padding = vec![0u8; 4096 - (compressed.len() + 4)];
        file.write_all(&padding)?;

        Ok(())
    }

    #[test]
    fn test_header_reading() -> Result<(), Box<dyn std::error::Error>> {
        create_test_mca("test.mca")?;

        let file = File::open("test.mca")?;
        let mut reader = BufReader::new(file);
        let headers = read_header(&mut reader)?;

        // test header for first chunk
        let header_entry = headers[0].as_ref().unwrap();
        assert_eq!(header_entry.sector_offset, 2);
        assert_eq!(header_entry.sector_count, 1);
        assert_eq!(header_entry.timestamp, 1);

        // test header for second chunk should be empty
        let header_entry = headers[1].as_ref();
        assert!(header_entry.is_none());

        Ok(())
    }

    #[test]
    fn test_mca_file_reading() -> Result<(), Box<dyn std::error::Error>> {
        create_test_mca("test.mca")?;

        let mca = MCAReader::from_file("test.mca", false)?;

        // test first chunk
        let chunk = mca.chunks[0].clone();
        match chunk {
            LazyChunk::Some(chunk) => {
                assert_eq!(chunk.timestamp, 1);
                assert!(!chunk.nbt.is_empty());
            }
            _ => panic!("Chunk should be Some"),
        }

        // test second chunk should be empty
        let chunk = mca.chunks[1].clone();
        match chunk {
            LazyChunk::NotExists => (),
            _ => panic!("Chunk should be NotExists"),
        }

        Ok(())
    }
    #[test]
    fn test_fastnbt_works() -> Result<(), Box<dyn std::error::Error>> {
        use fastnbt::{Value, nbt};
        let x = nbt!({
            "string": "Hello World",
            "number": 42,
            "nested": {
                "array": [1, 2, 3, 4, 5],
                "compound": {
                    "name": "test",
                    "value": 3.14,
                    "list": ["a", "b", "c"]
                }
            },
            "boolean": 1_i8,
            "long_array": [1_i64, 2_i64, 3_i64]
        });

        let y = fastnbt::to_bytes(&x)?;
        let z: Value = fastnbt::from_bytes(&y)?;
        let w = fastnbt::to_bytes(&z)?;
        assert_eq!(y, w);
        assert_eq!(format!("{:?}", x), format!("{:?}", z));

        Ok(())
    }

}
