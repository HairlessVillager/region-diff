use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::{commands::graph::EdgeCost, util::create_bincode_config};

use super::{Object, ObjectHash, tree::RelativeFilePath};
use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};

pub type Message = String;
pub type Timestamp = String; // todo: replace with DateTime<Utc>
type CommitHash = ObjectHash;
type TreeHash = ObjectHash;

#[derive(Debug, Encode, Decode)]
pub struct Commit {
    bare_tree: Option<ObjectHash>,
    parent_edges: HashMap<CommitHash, (TreeHash, EdgeCost)>,
    file_hashs: BTreeMap<RelativeFilePath, Vec<u8>>,
    message: Message,
    timestamp: Timestamp,
}

impl Commit {
    pub fn new(file_hashs: BTreeMap<RelativeFilePath, Vec<u8>>, message: String) -> Self {
        Self {
            bare_tree: None,
            parent_edges: HashMap::new(),
            file_hashs,
            message,
            timestamp: chrono::Utc::now().to_rfc2822(),
        }
    }
    pub fn add_parent(&mut self, commit: ObjectHash, tree: ObjectHash) {
        let cost = EdgeCost {
            patch: 1,
            revert: 1,
        }; // todo: replace with real cost
        self.parent_edges.insert(commit, (tree, cost));
    }
    pub fn from_bare(
        tree: ObjectHash,
        file_hashs: BTreeMap<RelativeFilePath, Vec<u8>>,
        message: String,
    ) -> Self {
        Self {
            bare_tree: Some(tree),
            parent_edges: HashMap::new(),
            file_hashs,
            message,
            timestamp: chrono::Utc::now().to_rfc2822(),
        }
    }
    pub fn get_message(&self) -> &Message {
        &self.message
    }
    pub fn get_timestamp(&self) -> &Timestamp {
        &self.timestamp
    }
    pub fn get_edges(&self) -> &HashMap<CommitHash, (TreeHash, EdgeCost)> {
        &self.parent_edges
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
