use super::ObjectHash;

struct Commit {
    upstreams: Vec<ObjectHash>,
    tree: ObjectHash,
    metadata: CommitMetadata,
}

struct CommitMetadata {}
