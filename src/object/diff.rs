use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};

use crate::{
    diff::{Diff as DiffTrait, base::BlobDiff, file::MCADiff},
    util::create_bincode_config,
};

#[derive(Debug, Encode, Decode)]
#[repr(u8)]
pub enum Diff {
    Blob(BlobDiff) = 1,
    Region(MCADiff) = 2,
}

impl ToString for Diff {
    fn to_string(&self) -> String {
        match self {
            Diff::Blob(_) => "blob",
            Diff::Region(_) => "region",
        }
        .to_string()
    }
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
        todo!()
    }

    pub fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
        todo!()
    }

    pub fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
        todo!()
    }

    pub fn serialize(&self) -> Vec<u8> {
        encode_to_vec(self, create_bincode_config()).unwrap()
    }

    pub fn deserialize(data: &Vec<u8>) -> Self {
        decode_from_slice(data, create_bincode_config())
            .map(|(de, _)| de)
            .unwrap()
    }
}
