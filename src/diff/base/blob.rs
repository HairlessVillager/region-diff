use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};

use crate::{
    diff::Diff,
    object::{Serde, SerdeError},
    util::create_bincode_config,
};

// Blob is one kind of git object, another two: Tree, Commit.
//
// Blob object in git stores the complete content of the file. The differences
// (diff) in Git are usually calculated on demand.
#[derive(Debug, Encode, Decode, Clone)]
pub struct BlobDiff {
    old_text: Vec<u8>,
    new_text: Vec<u8>,
}

impl Diff for BlobDiff {
    fn from_compare(old: &[u8], new: &[u8]) -> Self {
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

    fn patch(&self, old: &[u8]) -> Vec<u8> {
        let _ = old;
        self.new_text.clone()
    }

    fn revert(&self, new: &[u8]) -> Vec<u8> {
        let _ = new;
        self.old_text.clone()
    }
}

impl Serde for BlobDiff {
    fn serialize(&self) -> Result<Vec<u8>, SerdeError> {
        encode_to_vec(self, create_bincode_config()).map_err(|e| SerdeError::from(e))
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, SerdeError>
    where
        Self: Sized,
    {
        let result: Result<(BlobDiff, usize), _> =
            decode_from_slice(bytes, create_bincode_config());
        result
            .map(|(diff, _)| diff)
            .map_err(|e| SerdeError::from(e))
    }
}

impl BlobDiff {
    pub fn new() -> Self {
        Self::from_compare(&[], &[])
    }
    pub fn get_old_text(&self) -> &[u8] {
        &self.old_text
    }
    pub fn get_new_text(&self) -> &[u8] {
        &self.new_text
    }
    pub fn is_creation(&self) -> bool {
        self.old_text.len() == 0
    }
    pub fn is_deletion(&self) -> bool {
        self.new_text.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bytes(seed: u64) -> impl Iterator<Item = Vec<u8>> {
        use rand::prelude::*;
        let mut rng = StdRng::seed_from_u64(seed);

        std::iter::repeat_with(move || {
            let len = rng.random_range(0..10);
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                bytes.push(rng.random_range(0..3) as u8);
            }
            bytes
        })
    }
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

    #[test]
    fn test_serialize_deserialize() {
        let mut old_iter = create_test_bytes(114514);
        let mut new_iter = create_test_bytes(1919810);
        for _ in 0..100_000 {
            let old = old_iter.next().unwrap();
            let new = new_iter.next().unwrap();
            let diff = BlobDiff::from_compare(&old, &new);
            let serialized = diff.serialize().unwrap();
            let deserialized = BlobDiff::deserialize(&serialized).unwrap();
            assert_eq!(diff.old_text, deserialized.old_text);
            assert_eq!(diff.new_text, deserialized.new_text);
        }
    }
}
