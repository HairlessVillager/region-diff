use std::collections::{HashMap, HashSet};

use crate::{
    object::{ commit::Commit, index::Index, Object, ObjectHash, INDEX_HASH},
    storage::{StorageBackend, WrappedStorageBackend},
};

type SingleDirectedGraph = HashMap<ObjectHash, HashMap<ObjectHash, i32>>;

// todo: tooooo many .clone(), use Graph { node, patching, reverting } instead
pub fn graph(backend: &WrappedStorageBackend) -> (SingleDirectedGraph, SingleDirectedGraph) { // patching, reverting
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);

    let mut refs_stack: Vec<ObjectHash> = index
        .get_all_refs()
        .into_iter()
        .map(|e| e.clone())
        .collect();
    let mut visited = HashSet::new();
    let mut patching_graph: SingleDirectedGraph = HashMap::new();
    let mut reverting_graph: SingleDirectedGraph = HashMap::new();
    while let Some(top) = refs_stack.pop() {
        if visited.contains(&top) {
            continue;
        }
        let commit = backend.get(&top).unwrap();
        let commit = Commit::deserialize(&commit);
        let edges = commit.get_edges();
        let mut reverting_edges = HashMap::new();
        for edge in edges {
            if !visited.contains(&edge.commit) {
                refs_stack.push(edge.commit.clone());
            }
            reverting_edges.insert(edge.commit.clone(), edge.revert_cost);
            // reverting_graph.entry(&top).or_insert(HashMap::from([(edge.commit.clone(), edge.revert_cost)]));
            patching_graph.entry(edge.commit.clone()).and_modify(|edges|{edges.insert(top.clone(), edge.patch_cost);});
        }
        reverting_graph.insert(top.clone(), reverting_edges);
        visited.insert(top);
    }
    (patching_graph, reverting_graph)
}

pub fn weighted_shorted_path() {
    todo!()
}
