use blake2::{Blake2s256, Digest};

pub mod commit;
pub mod diff;
pub mod tree;

pub type ObjectHash = Vec<u8>; // 256 bits

pub fn object_hash(data: &Vec<u8>) -> ObjectHash {
    let mut hasher = Blake2s256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
