mod builder;
mod reader;
use std::fmt::Debug;

pub use builder::MCABuilder;
use fastnbt::Value;
pub use reader::{LazyChunk, MCAReader};

pub const SECTOR_SIZE: usize = 4096;
pub const LARGE_FLAG: u8 = 0b_1000_0000;

#[derive(Debug, Clone)]
struct HeaderEntry {
    idx: usize,
    sector_offset: u32,
    sector_count: u8,
    timestamp: u32,
}
impl HeaderEntry {
    #[allow(dead_code)]
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
pub enum ChunkNbt {
    Small(Vec<u8>),
    Large, // so large that saved to a extra .mcc file, see also: https://minecraft.wiki/w/Region_file_format#Payload
}

impl PartialEq for ChunkNbt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ChunkNbt::Large, ChunkNbt::Large) => true,
            (ChunkNbt::Small(self_nbt), ChunkNbt::Small(other_nbt)) => {
                fastnbt::from_bytes::<Value>(&self_nbt) == fastnbt::from_bytes::<Value>(&other_nbt)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkWithTimestamp {
    pub timestamp: u32,
    pub nbt: ChunkNbt,
}
