use std::{collections::BTreeMap, fs, path::PathBuf};

use walkdir::WalkDir;

use crate::{
    config::get_config,
    object::tree::{Tree, TreeBuildItem},
    storage::create_storage_backend,
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

pub fn commit(message: &str) {
    let config = get_config();
    let mut backend = create_storage_backend(&config.backend_url);
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
    Tree::from_iter(&mut backend, build_items);
    todo!("write tree object to storage backend");
    todo!("write commit object to storage backend");
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, init_config};

    use super::*;

    #[test]
    #[ignore = "todo: write tree object to storage backend; write commit object to storage backend"]
    fn test_commit() {
        init_config(Config {
            backend_url: "memory://".to_string(),
            base_dir: PathBuf::from("./resources/save/20250511"),
            working_dir: PathBuf::from("./resources/save/20250512"),
            log_config: crate::config::LogConfig::Development,
        });

        commit("test commit");
    }
}
