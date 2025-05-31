use std::collections::{BTreeMap, HashSet};

use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};
use blake2::{Blake2s256, Digest};

use crate::util::create_bincode_config;

use super::{INDEX_HASH, Object, ObjectHash};

#[derive(Debug, Encode, Decode)]
pub struct Index {
    refs: BTreeMap<String, ObjectHash>,
    head: Head,
}

#[derive(Debug, Encode, Decode)]
pub enum Head {
    Detached(ObjectHash),
    OnBranch(String),
}

impl Index {
    pub fn new(head: ObjectHash, branch: String) -> Self {
        let mut refs = BTreeMap::new();
        refs.insert(branch.clone(), head);
        Self {
            refs,
            head: Head::OnBranch(branch.clone()),
        }
    }
    pub fn set_head(&mut self, head: Head) {
        self.head = head;
    }
    pub fn set_ref(&mut self, name: String, commit: ObjectHash) {
        self.refs.insert(name, commit);
    }
    pub fn get_head(&self) -> &Head {
        &self.head
    }
    pub fn get_ref(&self, name: &String) -> Option<&ObjectHash> {
        self.refs.get(name)
    }
    pub fn get_all_refs(&self) -> HashSet<&ObjectHash> {
        let mut refs = HashSet::with_capacity(self.refs.len() + 1);
        refs.insert(match &self.head {
            Head::Detached(commit_hash) => commit_hash,
            Head::OnBranch(branch) => self.get_ref(branch).unwrap(),
        });
        for commit_hash in self.refs.values() {
            refs.insert(commit_hash);
        }
        refs
    }
}

impl Object for Index {
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
        let k = INDEX_HASH;
        let v = self.serialize();
        let mut hasher = Blake2s256::new();
        hasher.update(&v);
        (k.to_vec(), v)
    }
}
