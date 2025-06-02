use crate::{
    object::{
        INDEX_HASH, Object, ObjectHash,
        commit::{Commit, Message, Timestamp},
        index::{Head, Index},
    },
    storage::{StorageBackend, WrappedStorageBackend},
};

pub fn log(backend: &WrappedStorageBackend) -> Vec<(Message, Timestamp, ObjectHash)> {
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);

    let mut curr: ObjectHash = match index.get_head() {
        Head::Detached(commit_hash) => commit_hash,
        Head::OnBranch(branch) => index.get_ref(branch).unwrap(),
    }
    .clone();
    let mut prevs = Vec::new();
    loop {
        let commit = backend.get(&curr).unwrap();
        let commit = Commit::deserialize(&commit);
        log::trace!("commit: {:?}", commit);
        let message = commit.get_message().clone();
        let timestamp = commit.get_timestamp().clone();
        prevs.push((message, timestamp, curr));

        let first_prev = commit.get_edges().iter().nth(0); // todo: linear here, should in graph
        match first_prev {
            None => break,
            Some((edge_commit, _)) => {
                curr = edge_commit.clone();
            }
        }
    }
    prevs
}
