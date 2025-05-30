use std::collections::BTreeMap;

use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};
use blake2::{Blake2s256, Digest};
use rand::seq::index;

use crate::util::create_bincode_config;

use super::{INDEX_HASH, Object, ObjectHash};

#[derive(Debug, Encode, Decode)]
pub struct IndexV1 {
    refs: BTreeMap<String, ObjectHash>,
    head: ObjectHash,
}

pub enum Index {
    V0(IndexV1),
}

impl IndexV1 {
    pub fn new(head: ObjectHash) -> Self {
        Self {
            refs: BTreeMap::new(),
            head,
        }
    }
    pub fn update_head(&mut self, head: ObjectHash) {
        self.head = head;
    }
    pub fn add_ref(&mut self, name: String, commit: ObjectHash) {
        self.refs.insert(name, commit);
    }
}
impl Index {
    pub fn new(head: ObjectHash) -> Self {
        Self::V0(IndexV1::new(head))
    }
    pub fn update_head(&mut self, head: ObjectHash) {
        match self {
            Index::V0(index) => index.update_head(head),
        }
    }
    pub fn add_ref(&mut self, name: String, commit: ObjectHash) {
        match self {
            Index::V0(index) => index.add_ref(name, commit),
        }
    }
}
impl Object for Index {
    fn serialize(&self) -> Vec<u8> {
        let (header, body) = match self {
            Self::V0(index) => (0u8, encode_to_vec(index, create_bincode_config()).unwrap()),
        };
        [vec![header], body].concat()
    }
    fn deserialize(data: &Vec<u8>) -> Self {
        let header = data[0];
        let body = &data[1..];
        match header {
            0u8 => {
                let index = decode_from_slice(body, create_bincode_config())
                    .map(|(de, _)| de)
                    .unwrap();
                Self::V0(index)
            }
            h => panic!("unsupported index header: {}", h),
        }
    }
    fn as_kv(&self) -> (ObjectHash, Vec<u8>) {
        let k = INDEX_HASH;
        let v = self.serialize();
        let mut hasher = Blake2s256::new();
        hasher.update(&v);
        (k.to_vec(), v)
    }
}
