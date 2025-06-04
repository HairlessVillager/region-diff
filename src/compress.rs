use std::{
    fmt,
    io::{self, Cursor, Read, Write},
    str::FromStr,
};

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum CompressionType {
    /// GZip (RFC1952)
    Gzip,
    /// Zlib (RFC1950)
    Zlib,
    /// Uncompressed
    No,
    /// LZ4
    LZ4,
}

impl FromStr for CompressionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "gzip" => Ok(Self::Gzip),
            "zlib" => Ok(Self::Zlib),
            "no" => Ok(Self::No),
            "lz4" => Ok(Self::LZ4),
            _ => Err(format!("Invalid value: {}", s)),
        }
    }
}

impl fmt::Display for CompressionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Gzip => "GZip",
                Self::Zlib => "Zlib",
                Self::No => "No",
                Self::LZ4 => "LZ4",
            }
        )
    }
}

impl CompressionType {
    pub fn to_magic(&self) -> u8 {
        match self {
            CompressionType::Gzip => 1,
            CompressionType::Zlib => 2,
            CompressionType::No => 3,
            CompressionType::LZ4 => 4,
        }
    }
    pub fn from_magic(magic: u8) -> Self {
        match magic {
            1 => CompressionType::Gzip,
            2 => CompressionType::Zlib,
            3 => CompressionType::No,
            4 => CompressionType::LZ4,
            _ => panic!("unsupported compression type/magic"),
        }
    }
    pub fn compress_all<T: AsRef<[u8]>>(
        &self,
        data: T,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut reader = Cursor::new(data);
        let mut result = Vec::new();
        let mut writer = Cursor::new(&mut result);
        self.compress(&mut reader, &mut writer)?;
        Ok(result)
    }
    pub fn decompress_all<T: AsRef<[u8]>>(
        &self,
        data: T,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut reader = Cursor::new(data);
        let mut result = Vec::new();
        let mut writer = Cursor::new(&mut result);
        self.decompress(&mut reader, &mut writer)?;
        Ok(result)
    }
    pub fn compress(
        &self,
        input: &mut impl Read,
        output: &mut impl Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            CompressionType::Gzip => {
                let mut encoder =
                    flate2::write::GzEncoder::new(output, flate2::Compression::default());
                io::copy(input, &mut encoder)?;
                encoder.finish()?;
            }
            CompressionType::Zlib => {
                let mut encoder =
                    flate2::write::ZlibEncoder::new(output, flate2::Compression::default());
                io::copy(input, &mut encoder)?;
                encoder.finish()?;
            }
            CompressionType::No => {
                io::copy(input, output)?;
            }
            CompressionType::LZ4 => {
                let mut encoder = lz4_flex::frame::FrameEncoder::new(output);
                io::copy(input, &mut encoder)?;
                encoder.finish()?;
            }
        }
        Ok(())
    }
    pub fn decompress(
        &self,
        input: &mut impl Read,
        output: &mut impl Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            CompressionType::Gzip => {
                let mut decoder = flate2::write::GzDecoder::new(output);
                io::copy(input, &mut decoder)?;
                decoder.finish()?;
            }
            CompressionType::Zlib => {
                let mut decoder = flate2::write::ZlibDecoder::new(output);
                io::copy(input, &mut decoder)?;
                decoder.finish()?;
            }
            CompressionType::No => {
                io::copy(input, output)?;
            }
            CompressionType::LZ4 => {
                let mut decoder = lz4_flex::frame::FrameDecoder::new(input);
                io::copy(&mut decoder, output)?;
            }
        }
        Ok(())
    }
}
