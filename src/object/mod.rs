use blake2::{Blake2s256, Digest};

use crate::err::Error;

pub mod commit;
pub mod tree;
pub type ObjectHash = Vec<u8>; // 256 bits
pub trait Serde {
    fn serialize(&self) -> Result<Vec<u8>, Error>;
    fn deserialize(bytes: &Vec<u8>) -> Result<Self, Error>
    where
        Self: Sized;
}

pub fn object_hash(data: &Vec<u8>) -> ObjectHash {
    let mut hasher = Blake2s256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
