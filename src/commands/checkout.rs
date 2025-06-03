use crate::{
    config::get_config,
    object::{
        INDEX_HASH, Object,
        index::{Head, Index},
    },
    storage::{StorageBackend, WrappedStorageBackend},
};

use super::graph::create_graph;

pub fn checkout(backend: &mut WrappedStorageBackend, desired: &Head) {
    let config = get_config();
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);
    let graph = create_graph(backend);
    let current_commit = graph
        .get_commit(index.head_to_commit(index.get_head()))
        .unwrap();
    let desired_commit = graph.get_commit(index.head_to_commit(desired)).unwrap();
    let commit_path = graph.shortest_path(current_commit, desired_commit);
    todo!("traverse commit in commit_path, revert and patch");
}
