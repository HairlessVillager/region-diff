pub mod blob;
pub mod myers;

#[derive(Debug)]
struct DiffDesError {
    msg: String,
}

pub trait Diff {
    fn new() -> Self
    where
        Self: Sized;
    fn from_compare(old: &[u8], new: &[u8]) -> Self
    where
        Self: Sized;
    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized;
    fn patch(&self, old: &[u8]) -> Vec<u8>;
    fn revert(&self, new: &[u8]) -> Vec<u8>;
    fn serialize(&self) -> Result<Vec<u8>, ()>;
    fn deserialize(bytes: &[u8]) -> Result<Self, DiffDesError>
    where
        Self: Sized;
}
