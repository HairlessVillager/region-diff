use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
};

#[derive(Debug, Clone)]
struct HeaderEntry {
    idx: usize,
    sector_offset: u32,
    sector_count: u8,
    timestamp: u32,
}
impl HeaderEntry {
    fn is_available(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if self.sector_count == 0 && self.sector_offset == 0 {
            Ok(false)
        } else if self.sector_offset < 2 {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Sector {} overlaps with header", self.idx),
            )))
        } else if self.sector_count == 0 {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Sector {} size has to be > 0", self.idx),
            )))
        } else {
            Ok(true)
        }
    }
}
#[derive(Debug, Clone)]
pub enum LazyChunk {
    Unloaded,
    NotExists,
    Some(ChunkWithTimestamp),
}
#[derive(Debug, Clone)]
pub struct ChunkWithTimestamp {
    pub timestamp: u32,
    pub nbt: Vec<u8>,
}

pub struct MCAReader<R: Read + Seek> {
    mca_reader: R,
    header: [HeaderEntry; 1024],
    chunks: [LazyChunk; 1024],
}

impl<R: Read + Seek> MCAReader<R> {
    fn from_reader(mut reader: R, lazy: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let mut chunks = [const { LazyChunk::Unloaded }; 1024];
        let header = read_header(&mut reader)?;

        if !lazy {
            let mut header_refs: Vec<&HeaderEntry> = header.iter().collect();
            header_refs.sort_by_key(|e| e.sector_offset);
            for header_entry in header_refs {
                chunks[header_entry.idx] = match header_entry.sector_offset {
                    0 => LazyChunk::NotExists,
                    1..=u32::MAX => {
                        let offset = header_entry.sector_offset * 4096;
                        let _ = reader.seek(std::io::SeekFrom::Start(offset as u64));

                        let mut sector_buf = vec![0u8; header_entry.sector_count as usize * 4096];
                        reader.read_exact(&mut sector_buf).map_err(|e| {
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!(
                                    "Sector {} is out of bounds. Original error: {}",
                                    header_entry.idx, e
                                ),
                            ))
                        })?;
                        LazyChunk::Some(ChunkWithTimestamp {
                            timestamp: header_entry.timestamp,
                            nbt: read_chunk_nbt(&sector_buf)?,
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

    pub fn get_chunk(
        &mut self,
        x: usize,
        z: usize,
    ) -> Result<Option<&ChunkWithTimestamp>, Box<dyn std::error::Error>> {
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

        let mut sector_buf = vec![0u8; header.sector_count as usize * 4096];
        self.mca_reader
            .seek(SeekFrom::Start((header.sector_offset * 4096) as u64))?;
        self.mca_reader.read_exact(&mut sector_buf)?;

        let chunk = ChunkWithTimestamp {
            timestamp: header.timestamp,
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
    pub fn get_chunk_lazily(&self, x: usize, z: usize) -> &LazyChunk {
        let idx = x + 32 * z;
        &self.chunks[idx]
    }
}

impl MCAReader<BufReader<File>> {
    pub fn from_file(path: &str, lazy: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_reader(reader, lazy)
    }
}
impl<'a> MCAReader<Cursor<&'a Vec<u8>>> {
    pub fn from_bytes(bytes: &'a Vec<u8>) -> Result<Self, Box<dyn std::error::Error>> {
        let reader = Cursor::new(bytes);
        Self::from_reader(reader, false)
    }
}
fn read_header<R: Read + Seek>(
    reader: &mut R,
) -> Result<[HeaderEntry; 1024], Box<dyn std::error::Error>> {
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
    use std::io::Write;

    fn create_test_mca() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut buffer = Vec::new();
        let mut file = Cursor::new(&mut buffer);

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

        Ok(buffer)
    }

    #[test]
    fn test_header_reading() {
        let mut mca = create_test_mca().unwrap();
        let mut reader = Cursor::new(&mut mca);
        let headers = read_header(&mut reader).unwrap();

        // test header for first chunk
        let header_entry = &headers[0];
        assert_eq!(header_entry.sector_offset, 2);
        assert_eq!(header_entry.sector_count, 1);
        assert_eq!(header_entry.timestamp, 1);

        // test header for second chunk should be empty
        let header_entry = &headers[1];
        assert!(!header_entry.is_available().unwrap());
    }

    #[test]
    fn test_mca_file_reading() {
        let mut mca = create_test_mca().unwrap();
        let mca = MCAReader::from_bytes(&mut mca).unwrap();

        // test first chunk
        let chunk = mca.chunks[0].clone();
        match chunk {
            LazyChunk::Some(chunk) => {
                assert_eq!(chunk.timestamp, 1);
                assert!(!chunk.nbt.is_empty());
            }
            _ => panic!("Chunk should be Some, but got {:?}", chunk),
        }

        // test second chunk should be empty
        let chunk = mca.chunks[1].clone();
        match chunk {
            LazyChunk::NotExists => (),
            _ => panic!("Chunk should be NotExists, but got {:?}", chunk),
        }
    }

    #[test]
    fn test_real_files_reading() {
        let paths: Vec<&'static str> = vec![
            "./resources/mca/r.1.2.20250511.mca",
            "./resources/mca/r.1.2.20250512.mca",
            "./resources/mca/r.1.2.20250513.mca",
            "./resources/mca/r.1.2.20250514.mca",
            "./resources/mca/r.1.2.20250515.mca",
            "./resources/mca/r.1.2.20250516.mca",
        ];
        for path in paths {
            let mut reader = MCAReader::from_file(path, false).unwrap();
            for x in 0..32 {
                for z in 0..32 {
                    let _ = reader.get_chunk(x, z).unwrap();
                }
            }
        }
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
