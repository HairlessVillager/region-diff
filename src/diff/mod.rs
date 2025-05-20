mod full;
mod myers;

#[derive(Debug)]
struct DiffDesError {
    msg: String,
}

trait Diff {
    fn new() -> Self;
    fn from_compare(old: &[u8], new: &[u8]) -> Self;
    fn from_squash(base: &Self, squashing: &Self) -> Self;
    fn patch(&self, old: &[u8]) -> Vec<u8>;
    fn revert(&self, new: &[u8]) -> Vec<u8>;
    fn serialize(&self) -> Result<Vec<u8>, ()>;
    fn deserialize(bytes: &[u8]) -> Result<Self, DiffDesError>
    where
        Self: Sized;
}
