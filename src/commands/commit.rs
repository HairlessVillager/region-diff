use std::{collections::BTreeSet, fs, path::PathBuf};

use walkdir::WalkDir;

use crate::{
    config::Config,
    object::tree::{Tree, TreeBuildItem},
    storage::StorageBackend,
};

fn walkdir_strip_prefix(root: &PathBuf) -> BTreeSet<PathBuf> {
    BTreeSet::from_iter(
        WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|entry| {
                let path = entry.path();
                let relative_path = path.strip_prefix(root).unwrap_or(path);
                relative_path.into()
            }),
    )
}

pub fn commit<S: StorageBackend>(config: &Config<S>, message: &str) {
    let base = walkdir_strip_prefix(&config.base_dir);
    let working = walkdir_strip_prefix(&config.working_dir);
    let iter = base.union(&working).map(|path| {
        let f = |set: &BTreeSet<PathBuf>| {
            if set.contains(path) {
                fs::read(path).map(|c| Some(c)).expect(&format!(
                    "file {} exists but failed to read",
                    path.to_str().unwrap()
                ))
            } else {
                None
            }
        };
        TreeBuildItem {
            path: path.to_path_buf(),
            old: f(&base),
            new: f(&working),
        }
    });
    Tree::<S>::from_iter(&config.backend, &iter);
    todo!()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::storage::RocksDB;

    use super::*;

    #[test]
    #[ignore]
    fn test_commit() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path();

        let config = Config {
            backend: RocksDB::new(db_path).unwrap(),
            base_dir: PathBuf::from("./resources/save/20250511"),
            working_dir: PathBuf::from("./resources/save/20250512"),
        };
        commit(&config, "test commit");
        temp_dir.close().expect("Failed to clean up temp directory");
    }
}
