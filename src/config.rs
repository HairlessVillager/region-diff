use std::path::PathBuf;

use crate::storage::StorageBackend;

pub struct Config<S: StorageBackend> {
    pub(crate) backend: S,
    pub(crate) base_dir: PathBuf,
    pub(crate) working_dir: PathBuf,
}
