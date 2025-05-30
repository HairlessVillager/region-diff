use super::{Object, ObjectHash};
use bincode::{Decode, Encode};

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
