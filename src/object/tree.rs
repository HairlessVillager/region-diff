use walkdir::WalkDir;

use super::ObjectHash;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
type RelativeFilePath = PathBuf;

pub struct Tree {
    path2diff: BTreeMap<RelativeFilePath, ObjectHash>,
}
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
impl Tree {
    pub fn from_compare(base_path: &PathBuf, working_path: &PathBuf) {
        let base = walkdir_strip_prefix(base_path);
        let working = walkdir_strip_prefix(working_path);
        for path in base.union(&working) {
            match (base.contains(path), working.contains(path)) {
                (true, true) => todo!(),
                (true, false) => todo!(),
                (false, true) => todo!(),
                (false, false) => todo!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {}
