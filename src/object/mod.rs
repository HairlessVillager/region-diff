use blake2::{Blake2s256, Digest};

pub mod commit;
pub mod diff;
pub mod index;
pub mod tree;

pub type ObjectHash = Vec<u8>; // 256 bits
pub static INDEX_HASH: &'static [u8; 32] = &[0u8; 32];

fn object_hash(data: &Vec<u8>) -> ObjectHash {
    let mut hasher = Blake2s256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

pub trait Object {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(data: &Vec<u8>) -> Self;
    fn as_kv(&self) -> (ObjectHash, Vec<u8>) {
        let v = self.serialize();
        let mut hasher = Blake2s256::new();
        hasher.update(&v);
        let k = hasher.finalize().to_vec();
        (k, v)
    }
}
