use crate::storage::StorageBackend;
use glob::Pattern as GlobPattern;

use super::ObjectHash;
use std::{collections::BTreeMap, path::PathBuf};
type RelativeFilePath = PathBuf;

pub struct Tree<S: StorageBackend> {
    storage: S,
    path2diff: BTreeMap<RelativeFilePath, ObjectHash>,
}

pub struct TreeBuildItem {
    pub(crate) path: PathBuf,
    pub(crate) old: Option<Vec<u8>>,
    pub(crate) new: Option<Vec<u8>>,
}

struct Strategy {
    pattern: StrategyPattern,
    diff: StrategyDiff,
}
enum StrategyPattern {
    Glob(GlobPattern),
}
enum StrategyDiff {
    RegionDiff,
    BlobDiff,
}

impl<S: StorageBackend> Tree<S> {
    pub fn from_iter<I>(backend: &S, iter: &I) -> Self
    where
        I: Iterator<Item = TreeBuildItem>,
    {
        let strategies = vec![Strategy {
            pattern: StrategyPattern::Glob(GlobPattern::new("**/*.mca").unwrap()),
            diff: StrategyDiff::RegionDiff,
        }];

        todo!("write to backend")
    }
}

#[cfg(test)]
mod tests {}
