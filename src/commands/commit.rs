use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use walkdir::WalkDir;

use crate::{
    config::Config,
    object::tree::{Tree, TreeBuildItem},
    storage::StorageBackend,
    util::merge_map,
};

fn walkdir_strip_prefix(root: &PathBuf) -> BTreeMap<PathBuf, PathBuf> {
    BTreeMap::from_iter(
        WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|entry| {
                let path = entry.path();
                let relative_path = path.strip_prefix(root).unwrap_or(path);
                (relative_path.into(), path.into())
            }),
    )
}

pub fn commit<S: StorageBackend>(mut config: Config<S>, message: &str) {
    let base = walkdir_strip_prefix(&config.base_dir);
    let working = walkdir_strip_prefix(&config.working_dir);
    let base_working = merge_map(base, working);
    let build_items = base_working
        .into_iter()
        .map(|(rela, (abs_base, abs_working))| TreeBuildItem {
            path: rela.to_path_buf(),
            old: abs_base.map(|path| {
                fs::read(&path).expect(&format!("file {:?} exists but failed to read", &path))
            }),
            new: abs_working.map(|path| {
                fs::read(&path).expect(&format!("file {:?} exists but failed to read", &path))
            }),
        });
    Tree::from_iter(&mut config.backend, build_items);
    todo!("write tree object to storage backend");
    todo!("write commit object to storage backend");
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        log::init as init_log,
        storage::{RocksDB, WrappedStorageBackend},
    };

    use super::*;

    #[test]
    #[ignore = "todo: write tree object to storage backend; write commit object to storage backend"]
    fn test_commit() {
        init_log(crate::log::Config::Development).unwrap();

        log::debug!("test commit...");

        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path();

        let config = Config {
            backend: WrappedStorageBackend::new("memory://"),
            base_dir: PathBuf::from("./resources/save/20250511"),
            working_dir: PathBuf::from("./resources/save/20250512"),
        };
        commit(config, "test commit");

        temp_dir.close().expect("Failed to clean up temp directory");
    }
}
