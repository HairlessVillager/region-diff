use std::{collections::BTreeMap, fs, path::PathBuf};

use walkdir::WalkDir;

use crate::{
    config::get_config,
    object::{Commit, Head, INDEX_HASH, Index, Object, Tree, TreeBuildItem},
    storage::{StorageBackend, WrappedStorageBackend, create_storage_backend},
    util::{merge_map, put_object},
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

pub fn commit(backend: &mut WrappedStorageBackend, message: &str) {
    let config = get_config();
    if !config.base_dir.exists() {
        panic!("base directory {:?} not exists", config.base_dir);
    }
    if !config.working_dir.exists() {
        panic!("working directory {:?} not exists", config.working_dir);
    }

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

    let tree = Tree::from_iter(backend, build_items);
    let (tree_key, tree_value) = tree.as_kv();
    backend.put(&tree_key, &tree_value).unwrap();

    // not initial commit
    if backend.exists(INDEX_HASH) {
        log::trace!("not initial commit");
        let index = backend.get(INDEX_HASH).unwrap();
        let mut index = Index::deserialize(&index);

        match index.get_head() {
            Head::Detached(prev_commit_hash) => {
                log::trace!("head is Head::Detached");
                let commit =
                    Commit::from(Some(&vec![prev_commit_hash.clone()]), &tree_key, message);
                let (commit_key, commit_value) = commit.as_kv();
                backend.put(&commit_key, &commit_value).unwrap();

                index.set_head(Head::Detached(commit_key));
                let index = index.serialize();
                backend.put(INDEX_HASH, index).unwrap();
            }

            Head::OnBranch(branch) => {
                log::trace!("head is Head::OnBranch");
                let prev_commit_hash = index.get_ref(branch).unwrap();
                let commit =
                    Commit::from(Some(&vec![prev_commit_hash.clone()]), &tree_key, message);
                let (commit_key, commit_value) = commit.as_kv();
                backend.put(&commit_key, &commit_value).unwrap();

                index.set_ref(branch.clone(), commit_key);
                let index = index.serialize();
                backend.put(INDEX_HASH, index).unwrap();
            }
        }
    }
    // initial commit
    else {
        log::trace!("initial commit");
        let commit = Commit::from(None, &tree_key, message);
        let (commit_key, commit_value) = commit.as_kv();
        backend.put(&commit_key, &commit_value).unwrap();

        let index = Index::new(commit_key, "main".to_string()); // todo: configuable default branch name
        let index = index.serialize();
        backend.put(INDEX_HASH, index).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        commands::{log, status},
        config::{Config, with_test_config},
    };

    use super::*;

    #[test]
    fn test_commit() {
        let backend_url = "tempdir://";
        let mut backend = create_storage_backend(backend_url);
        let message_1 = "test commit #1";
        let message_2 = "test commit #2";

        with_test_config(
            Config {
                backend_url: backend_url.to_string(),
                base_dir: PathBuf::from("./resources/save/20250511"),
                working_dir: PathBuf::from("./resources/save/20250512"),
                log_config: crate::config::LogConfig::NoLog,
            },
            || {
                commit(&mut backend, message_1);

                let logs = log(&backend);
                assert_eq!(logs[0].0, message_1);
                let status = status(&backend);
                assert_eq!(status.0, Some("main".to_string()));
                assert_eq!(status.2, message_1);
            },
        );

        with_test_config(
            Config {
                backend_url: backend_url.to_string(),
                base_dir: PathBuf::from("./resources/save/20250512"),
                working_dir: PathBuf::from("./resources/save/20250513"),
                log_config: crate::config::LogConfig::Trace,
            },
            || {
                commit(&mut backend, message_2);

                let logs = log(&backend);
                assert_eq!(logs[0].0, message_2);
                assert_eq!(logs[1].0, message_1);
                let status = status(&backend);
                assert_eq!(status.0, Some("main".to_string()));
                assert_eq!(status.2, message_2);
            },
        );
    }
}
