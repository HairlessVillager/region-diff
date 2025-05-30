use std::{collections::BTreeMap, fs, path::PathBuf};

use walkdir::WalkDir;

use crate::{
    config::get_config,
    object::{Commit, INDEX_HASH, Index, Object, Tree, TreeBuildItem},
    storage::{StorageBackend, WrappedStorageBackend, create_storage_backend},
    util::{merge_map, put_object},
};

pub fn checkout(backend: &mut WrappedStorageBackend, target: Head) {}

#[cfg(test)]
mod tests {
    use crate::{
        commands::{log, status},
        config::{Config, init_config},
    };

    use super::*;

    #[test]
    #[ignore = "todo: assert commit log"]
    fn test_checkout() {
        init_config(Config {
            backend_url: "tempdir://".to_string(),
            base_dir: PathBuf::from("./resources/save/20250511"),
            working_dir: PathBuf::from("./resources/save/20250512"),
            log_config: crate::config::LogConfig::Trace,
        });
        let mut backend = create_storage_backend(&get_config().backend_url);

        let message = "test commit";
        commit(&mut backend, message);
        let logs = log(&backend);
        assert_eq!(logs[0].0, message);
        let status = status(&backend);
        assert_eq!(status.0, Some("main".to_string()));
        assert_eq!(status.2, message);
    }
}
