use std::collections::BTreeSet;

use crate::{commands::graph::EdgeCost, util::create_bincode_config};

use super::{Object, ObjectHash, tree::RelativeFilePath};
use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};

pub type Message = String;
pub type Timestamp = String; // todo: replace with DateTime<Utc>

#[derive(Debug, Encode, Decode)]
pub struct Commit {
    bare_tree: Option<ObjectHash>,
    parent_edges: Vec<ParentEdge>, // todo: use map
    files: BTreeSet<RelativeFilePath>,
    message: Message,
    timestamp: Timestamp,
}

#[derive(Debug, Encode, Decode, Clone)]
pub struct ParentEdge {
    pub commit: ObjectHash,
    pub tree: ObjectHash,
    pub cost: EdgeCost,
}

impl Commit {
    pub fn new(files: BTreeSet<RelativeFilePath>, message: String) -> Self {
        Self {
            bare_tree: None,
            parent_edges: Vec::with_capacity(0),
            files,
            message,
            timestamp: chrono::Utc::now().to_rfc2822(),
        }
    }
    pub fn add_parent(&mut self, commit: ObjectHash, tree: ObjectHash) {
        self.parent_edges.push(ParentEdge {
            commit,
            tree,
            cost: EdgeCost {
                patch: 1,
                revert: 1,
            },
        }); // todo: replace with real cost
    }
    pub fn from_bare(tree: ObjectHash, files: BTreeSet<RelativeFilePath>, message: String) -> Self {
        Self {
            bare_tree: Some(tree),
            parent_edges: Vec::with_capacity(0),
            files,
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
    pub fn get_edges(&self) -> &Vec<ParentEdge> {
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
