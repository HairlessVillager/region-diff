use std::{collections::BTreeMap, path::PathBuf};

use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};
use fastnbt::value;
use glob::Pattern as GlobPattern;
use hex::encode as hex;

use super::{ObjectHash, diff::Diff};
use crate::{
    object::{Object, object_hash},
    storage::StorageBackend,
    util::create_bincode_config,
};

type RelativeFilePath = PathBuf;

#[derive(Debug, Encode, Decode)]
pub struct Tree {
    path2diff: BTreeMap<RelativeFilePath, ObjectHash>,
}

#[derive(Debug, Encode, Decode)]
pub struct TreeBuildItem {
    pub(crate) path: PathBuf,
    pub(crate) old: Option<Vec<u8>>,
    pub(crate) new: Option<Vec<u8>>,
}

// TODO: rename to Policy
struct Strategy {
    pattern: Pattern,
    diff: String,
}

enum Pattern {
    Glob(GlobPattern),
}

impl Tree {
    pub fn from_iter<S, I>(backend: &mut S, build_items: I) -> Self
    where
        S: StorageBackend,
        I: Iterator<Item = TreeBuildItem>,
    {
        // TODO: configurable
        let strategies = vec![Strategy {
            pattern: Pattern::Glob(GlobPattern::new("*.mca").unwrap()),
            diff: "region".to_string(),
        }];
        let default_diff_type = "blob";

        let mut path2diff = BTreeMap::new();

        let tree_build_item_2_diff = |item: TreeBuildItem| match (item.old, item.new) {
            (None, None) => {
                log::warn!(
                    "{:?}: both old and new data not exist, will ignore",
                    item.path
                );
                None
            }
            (None, Some(new)) => Some((item.path, Diff::from_create(&new))),
            (Some(old), None) => Some((item.path, Diff::from_delete(&old))),
            (Some(old), Some(new)) => {
                let diff_type = strategies
                    .iter()
                    .find_map(|s| match &s.pattern {
                        Pattern::Glob(p) => {
                            if p.matches_path(&item.path) {
                                Some(s.diff.as_str())
                            } else {
                                None
                            }
                        }
                    })
                    .unwrap_or(default_diff_type);
                Some((item.path, Diff::from_compare(diff_type, &old, &new)))
            }
        };
        let diff_2_kv = |(path, diff): (PathBuf, Diff)| {
            let (key, value) = diff.as_kv();
            log::debug!("insert path: {:?}, key: {}", path, hex(&key));
            path2diff.insert(path, key.clone());
            (key, value)
        };

        let iter = build_items
            .filter_map(tree_build_item_2_diff)
            .map(diff_2_kv);
        backend.put_batch(iter).unwrap();

        Self { path2diff }
    }
}

impl Object for Tree {
    fn serialize(&self) -> Vec<u8> {
        encode_to_vec(self, create_bincode_config()).unwrap()
    }
    fn deserialize(data: &Vec<u8>) -> Self
    where
        Self: Decode<()>,
    {
        decode_from_slice(data, create_bincode_config())
            .map(|(de, _)| de)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {}
