mod builder;
mod reader;
use std::fmt::Debug;

pub use builder::MCABuilder;
use fastnbt::Value;
pub use reader::{LazyChunk, MCAReader};

pub const SECTOR_SIZE: usize = 4096;

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
            Err(format!("Sector {} overlaps with header", self.idx).into())
        } else if self.sector_count == 0 {
            Err(format!("Sector {} size has to be > 0", self.idx).into())
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
