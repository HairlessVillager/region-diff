use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};

use crate::{
    diff::{
        Diff as DiffTrait,
        base::{self, BlobDiff},
        file::MCADiff,
    },
    util::create_bincode_config,
};

use super::Object;

#[derive(Debug, Encode, Decode)]
pub enum Diff {
    Blob(BlobDiff),
    Region(MCADiff),
}

impl Diff {
    pub fn from_compare(diff_type: &str, old: &Vec<u8>, new: &Vec<u8>) -> Self {
        match diff_type {
            "blob" => Self::Blob(BlobDiff::from_compare(old, new)),
            "region" => Self::Region(MCADiff::from_compare(old, new)),
            _ => panic!("unsupport diff type"),
        }
    }

    pub fn from_create(new: &Vec<u8>) -> Self {
        Self::Blob(BlobDiff::from_create(new))
    }

    pub fn from_delete(old: &Vec<u8>) -> Self {
        Self::Blob(BlobDiff::from_delete(old))
    }

    pub fn from_squash(base: &Self, squashing: &Self) -> Self {
        match (base, squashing) {
            (Diff::Blob(base), Diff::Blob(squashing)) => {
                Self::Blob(BlobDiff::from_squash(base, squashing))
            }
            (Diff::Blob(base), Diff::Region(squashing)) => {
                let old = base.get_old_text();
                let new = &squashing.patch(base.get_new_text());
                Self::Blob(BlobDiff::from_compare(old, new))
            }
            (Diff::Region(base), Diff::Blob(squashing)) => {
                let old = &base.revert(squashing.get_old_text());
                let new = squashing.get_new_text();
                Self::Blob(BlobDiff::from_compare(old, new))
            }
            (Diff::Region(base), Diff::Region(squashing)) => {
                Self::Region(MCADiff::from_squash(base, squashing))
            }
        }
    }

    pub fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
        match self {
            Self::Blob(diff) => diff.patch0(),
            Self::Region(diff) => diff.patch(old),
        }
    }

    pub fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
        match self {
            Self::Blob(diff) => diff.revert0(),
            Self::Region(diff) => diff.revert(new),
        }
    }
}

impl Object for Diff {
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
