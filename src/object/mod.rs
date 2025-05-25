use blake2::{Blake2s256, Digest};

pub mod commit;
pub mod tree;
pub type ObjectHash = Vec<u8>; // 256 bits
#[derive(Debug)]
pub struct SerdeError {
    msg: String,
}
impl SerdeError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
    pub fn from<E: ToString>(err: E) -> Self {
        Self::new(err.to_string())
    }
}
pub trait Serde {
    fn serialize(&self) -> Result<Vec<u8>, SerdeError>;
    fn deserialize(bytes: &Vec<u8>) -> Result<Self, SerdeError>
    where
        Self: Sized;
}

pub fn object_hash(data: &Vec<u8>) -> ObjectHash {
    let mut hasher = Blake2s256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
