use bincode::{Decode, Encode};

use crate::diff::Diff;

// Blob is one kind of git object, another two: Tree, Commit.
//
// Blob object in git stores the complete content of the file. The differences
// (diff) in Git are usually calculated on demand.
#[derive(Debug, Encode, Decode, Clone)]
pub struct BlobDiff {
    old_text: Vec<u8>,
    new_text: Vec<u8>,
}

impl Diff<Vec<u8>> for BlobDiff {
    fn from_compare(old: &Vec<u8>, new: &Vec<u8>) -> Self {
        Self {
            old_text: old.to_vec(),
            new_text: new.to_vec(),
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self {
        Self {
            old_text: base.old_text.clone(),
            new_text: squashing.new_text.clone(),
        }
    }

    fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
        let _ = old;
        self.new_text.clone()
    }

    fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
        let _ = new;
        self.old_text.clone()
    }
}
impl BlobDiff {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::from_compare(&Vec::with_capacity(0), &Vec::with_capacity(0))
    }
    pub fn from_create(new: &Vec<u8>) -> Self {
        Self::from_compare(&Vec::with_capacity(0), new)
    }
    pub fn from_delete(old: &Vec<u8>) -> Self {
        Self::from_compare(old, &Vec::with_capacity(0))
    }
    pub fn get_old_text(&self) -> &Vec<u8> {
        &self.old_text
    }
    pub fn get_new_text(&self) -> &Vec<u8> {
        &self.new_text
    }
    pub fn patch0(&self) -> Vec<u8> {
        self.new_text.clone()
    }
    pub fn revert0(&self) -> Vec<u8> {
        self.old_text.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::util::test::create_test_bytes;

    use super::*;

    #[test]
    fn test_diff_patch_revert() -> () {
        let mut old_iter = create_test_bytes(114514);
        let mut new_iter = create_test_bytes(1919810);
        for _ in 0..100_000 {
            let old = old_iter.next().unwrap();
            let new = new_iter.next().unwrap();
            let diff = BlobDiff::from_compare(&old, &new);
            let patched_old = diff.patch(&old);
            let reverted_new = diff.revert(&new);
            assert_eq!(patched_old, new, "old: {:?}; new: {:?}", old, new);
            assert_eq!(reverted_new, old, "old: {:?}; new: {:?}", old, new);
        }
    }
    #[test]
    fn test_diff_squash() -> () {
        let mut v0_iter = create_test_bytes(114514);
        let mut v1_iter = create_test_bytes(1919810);
        let mut v2_iter = create_test_bytes(19260817);
        for _ in 0..100_000 {
            let v0 = v0_iter.next().unwrap();
            let v1 = v1_iter.next().unwrap();
            let v2 = v2_iter.next().unwrap();
            let diff_v01 = BlobDiff::from_compare(&v0, &v1);
            let diff_v12 = BlobDiff::from_compare(&v1, &v2);
            let squashed_diff = BlobDiff::from_squash(&diff_v01, &diff_v12);
            let patched_v0 = squashed_diff.patch(&v0);
            let reverted_v2 = squashed_diff.revert(&v2);
            assert_eq!(patched_v0, v2, "v0: {:?}; v1{:?}; v2: {:?}", v0, v1, v2);
            assert_eq!(reverted_v2, v0, "v0: {:?}; v1{:?}; v2: {:?}", v0, v1, v2);
        }
    }
}
