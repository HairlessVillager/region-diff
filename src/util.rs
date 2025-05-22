use fastnbt::{Value, error::Error};

pub fn sort_value(bytes: &Vec<u8>) -> Result<Vec<u8>, Error> {
    let de: Value = fastnbt::from_bytes(&bytes)?;
    let sorted = fastnbt::to_bytes(&de)?;
    Ok(sorted)
}
