pub mod commit;
pub mod diff;
pub mod tree;
pub type ObjectHash = [u8; 32]; // 256 bits
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
    fn deserialize(bytes: &[u8]) -> Result<Self, SerdeError>
    where
        Self: Sized;
}
pub trait Object: Serde {
    fn hash(&self) -> ObjectHash;
}
