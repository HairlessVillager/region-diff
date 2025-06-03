use std::path::PathBuf;

use crate::object::index::Index;

pub fn base_check(base: &PathBuf, index: &Index) {
    todo!("check hashs of all files under base directory, assert they are equal to hashs in index");
}
