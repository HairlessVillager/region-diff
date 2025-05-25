mod builder;
mod reader;
use std::fmt::{Debug, format};

pub use builder::MCABuilder;
use fastnbt::Value;
pub use reader::{LazyChunk, MCAReader};

pub const SECTOR_SIZE: usize = 4096;
#[derive(Debug)]
pub struct MCAFileParsingError {
    msg: String,
}
impl MCAFileParsingError {
    pub fn from_msg(msg: String) -> Self {
        Self { msg }
    }
    pub fn from_err<E: Debug>(err: E) -> Self {
        Self::from_msg(format!("{:?}", err))
    }
    pub fn from_msg_err<E: Debug>(msg: &str, err: E) -> Self {
        Self::from_msg(format!("{}: {:?}", msg, err))
    }
}
#[derive(Debug, Clone)]
struct HeaderEntry {
    idx: usize,
    sector_offset: u32,
    sector_count: u8,
    timestamp: u32,
}
impl HeaderEntry {
    fn is_available(&self) -> Result<bool, MCAFileParsingError> {
        if self.sector_count == 0 && self.sector_offset == 0 {
            Ok(false)
        } else if self.sector_offset < 2 {
            Err(MCAFileParsingError::from_msg(format!(
                "Sector {} overlaps with header",
                self.idx
            )))
        } else if self.sector_count == 0 {
            Err(MCAFileParsingError::from_msg(format!(
                "Sector {} size has to be > 0",
                self.idx
            )))
        } else {
            Ok(true)
        }
    }
}
#[derive(Debug, Clone)]
pub struct ChunkWithTimestamp {
    pub timestamp: u32,
    pub nbt: Vec<u8>,
}

impl PartialEq for ChunkWithTimestamp {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && fastnbt::from_bytes::<Value>(&self.nbt) == fastnbt::from_bytes::<Value>(&other.nbt)
    }
}
