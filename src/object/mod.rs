pub mod commit;
pub mod diff;
pub mod tree;
type ObjectHash = [u8; 32]; // 256 bits
#[derive(Debug)]
struct ObjectDeserializieError {
    msg: String,
}
trait Object {
    fn serialize(&self) -> Result<Vec<u8>, ()>;
    fn deserialize(bytes: &[u8]) -> Result<Self, ObjectDeserializieError>
    where
        Self: Sized;
    fn hash(&self) -> ObjectHash;
}
