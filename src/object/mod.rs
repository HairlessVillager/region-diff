pub mod commit;
pub mod diff;
pub mod tree;
pub type ObjectHash = [u8; 32]; // 256 bits
#[derive(Debug)]
pub struct BytesSerDeError {
    msg: String,
}
impl BytesSerDeError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}
pub trait BytesSerDe {
    fn serialize(&self) -> Result<Vec<u8>, ()>;
    fn deserialize(bytes: &[u8]) -> Result<Self, BytesSerDeError>
    where
        Self: Sized;
}
pub trait Object: BytesSerDe {
    fn hash(&self) -> ObjectHash;
}
