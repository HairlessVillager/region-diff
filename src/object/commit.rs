use crate::util::create_bincode_config;

use super::{Object, ObjectHash};
use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};

#[derive(Debug, Encode, Decode)]
pub struct Commit {
    parents: Vec<ObjectHash>,
    tree: ObjectHash,
    message: String,
    timestamp: String,
}

impl Commit {
    pub fn from(parents: Option<&Vec<ObjectHash>>, tree: &ObjectHash, message: &str) -> Self {
        Self {
            parents: parents.map(|x| x.clone()).unwrap_or(Vec::new()),
            tree: tree.clone(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().format("%H:%M:%S%.6f").to_string(),
        }
    }
}

impl Object for Commit {
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
