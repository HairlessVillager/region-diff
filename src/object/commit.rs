use std::collections::{BTreeMap, HashMap};

use super::diff::Diff;
type RelativeFilePath = String;

struct Commit {
    tree: BTreeMap<RelativeFilePath, Box<dyn Diff>>,
}
pub struct CommitGraph {
    adjacency_list: HashMap,
}
