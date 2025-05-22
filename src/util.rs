pub fn rearranged_nbt(bytes: &Vec<u8>) -> Result<Vec<u8>, fastnbt::error::Error> {
    let de: fastnbt::Value = fastnbt::from_bytes(&bytes)?;
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

pub fn create_bincode_config() -> bincode::config::Configuration<bincode::config::BigEndian> {
    bincode::config::standard()
        .with_big_endian()
        .with_variable_int_encoding()
}
