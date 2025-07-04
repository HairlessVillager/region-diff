mod builder;
mod reader;
use std::fmt::Debug;
use thiserror::Error;

pub use builder::MCABuilder;
pub use reader::{LazyChunk, MCAReader};

use crate::util::nbt_serde::de;

pub const SECTOR_SIZE: usize = 4096;
pub const LARGE_FLAG: u8 = 0b_1000_0000;

#[derive(Error, Debug)]
pub enum MCAError {
    #[error("Sector {idx} overlaps with header")]
    SectorHeaderOverlap { idx: usize },
    #[error("Sector {idx} size has to be > 0")]
    InvalidSectorSize { idx: usize },
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Compression error in chunk ({x}, {z}): {reason}")]
    Compression { x: usize, z: usize, reason: String },
    #[error("NBT parsing error in chunk ({x}, {z}): {source}")]
    NBTParsingError {
        x: usize,
        z: usize,
        source: fastnbt::error::Error,
    },
    #[error("Failed to load chunk at ({x}, {z}): {reason}")]
    ChunkLoadFailed { x: usize, z: usize, reason: String },
}

#[derive(Debug, Clone)]
struct HeaderEntry {
    idx: usize,
    sector_offset: u32,
    sector_count: u8,
    timestamp: u32,
}
impl HeaderEntry {
    #[allow(dead_code)]
    fn is_available(&self) -> Result<bool, MCAError> {
        if self.sector_count == 0 && self.sector_offset == 0 {
            Ok(false)
        } else if self.sector_offset < 2 {
            Err(MCAError::SectorHeaderOverlap { idx: self.idx })
        } else if self.sector_count == 0 {
            Err(MCAError::InvalidSectorSize { idx: self.idx })
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
                de(&self_nbt) == de(&other_nbt)
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
