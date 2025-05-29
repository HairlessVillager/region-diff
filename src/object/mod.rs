use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};
use blake2::{Blake2s256, Digest};

use crate::util::create_bincode_config;

pub mod commit;
pub mod diff;
pub mod tree;

pub type ObjectHash = Vec<u8>; // 256 bits

fn object_hash(data: &Vec<u8>) -> ObjectHash {
    let mut hasher = Blake2s256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

trait Object: Encode + Decode<()> {
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
    fn as_kv(&self) -> (ObjectHash, Vec<u8>) {
        let v = self.serialize();
        let mut hasher = Blake2s256::new();
        hasher.update(&v);
        let k = hasher.finalize().to_vec();
        (k, v)
    }
}
