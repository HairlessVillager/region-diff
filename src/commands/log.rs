use crate::{
    object::{commit::{Commit, Message, Timestamp}, index::{Head, Index}, Object, ObjectHash, INDEX_HASH},
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

        let prev = commit.get_edges().get(0); // todo: line here, should in graph
        match prev {
            None => break,
            Some(edge) => {
                curr = edge.commit.clone();
            }
        }
    }
    prevs
}
