use crate::{
    object::{Commit, Head, INDEX_HASH, Index, Message, Object, ObjectHash, Timestamp},
    storage::{StorageBackend, WrappedStorageBackend, create_storage_backend},
};

pub fn status(backend: &WrappedStorageBackend) -> (Option<String>, ObjectHash, Message, Timestamp) {
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);

    let name;
    let curr: ObjectHash = match index.get_head() {
        Head::Detached(commit_hash) => {
            name = None;
            commit_hash
        }
        Head::OnBranch(branch) => {
            name = Some(branch.clone());
            index.get_ref(branch).unwrap()
        }
    }
    .clone();

    let commit = backend.get(&curr).unwrap();
    let commit = Commit::deserialize(&commit);
    let message = commit.get_message().clone();
    let timestamp = commit.get_timestamp().clone();

    (name, curr, message, timestamp)
}
