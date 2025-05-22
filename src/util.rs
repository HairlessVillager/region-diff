use fastnbt::{Value, error::Error};

use crate::mca::{CompressionType, LazyChunk, MCABuilder, MCAReader};

pub fn rearranged_nbt(bytes: &Vec<u8>) -> Result<Vec<u8>, Error> {
    let de: Value = fastnbt::from_bytes(&bytes)?;
    let sorted = fastnbt::to_bytes(&de)?;
    Ok(sorted)
}

pub fn create_chunk_ixz_iter() -> impl Iterator<Item = (usize, usize, usize)> {
    (0..32).flat_map(|z| {
        (0..32).map(move |x| {
            let i = x + 32 * z;
            (i, x, z)
        })
    })
}
