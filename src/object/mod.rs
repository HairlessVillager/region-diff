use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};
use blake2::{Blake2s256, Digest};

use crate::util::create_bincode_config;

mod commit;
mod diff;
mod index;
mod tree;

pub use commit::Commit;
pub use diff::Diff;
pub use index::Index;
pub use tree::{Tree, TreeBuildItem};

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

impl<T: Encode + Decode<()>> Object for T {
    fn serialize(&self) -> Vec<u8> {
        encode_to_vec(self, create_bincode_config()).unwrap()
    }
    fn deserialize(data: &Vec<u8>) -> Self
    where
        Self: Decode<()>,
    {
        decode_from_slice(data, create_bincode_config())
            .map(|(de, _)| de)
            .unwrap()
    }
}
