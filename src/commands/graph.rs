use petgraph::algo::dijkstra;
use petgraph::graph::{Graph, NodeIndex};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    object::{Commit, Head, INDEX_HASH, Index, Object, ObjectHash},
    storage::{StorageBackend, WrappedStorageBackend},
};

pub fn graph(backend: &WrappedStorageBackend) -> HashMap<ObjectHash, Vec<ObjectHash>> {
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);

    let mut refs_stack: Vec<ObjectHash> = index
        .get_all_refs()
        .into_iter()
        .map(|e| e.clone())
        .collect();
    let mut graph = HashMap::new();
    while let Some(top) = refs_stack.pop() {
        let commit = backend.get(&top).unwrap();
        let commit = Commit::deserialize(&commit);
        let parents = commit.get_parents().clone();
        for parnet in &parents {
            if !graph.contains_key(parnet) {
                refs_stack.push(parnet.clone());
            }
        }
        graph.insert(top.clone(), parents);
    }
    graph
}

pub fn weighted_shorted_path() {
    todo!()
}
