use std::io::{Cursor, Read};

use super::{Diff, DiffDesError};

// Blob is one kind of git object, another two: Tree, Commit.
//
// Blob object in git stores the complete content of the file. The differences
// (diff) in Git are usually calculated on demand.
#[derive(Debug)]
pub struct BlobDiff {
    old_text: Vec<u8>,
    new_text: Vec<u8>,
}

impl Diff for BlobDiff {
    fn new() -> Self {
        Self {
            old_text: vec![],
            new_text: vec![],
        }
    }
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

    fn serialize(&self) -> Result<Vec<u8>, ()> {
        let capacity_bytes = 32 + self.old_text.len() + self.new_text.len();
        let mut buffer: Vec<u8> = Vec::with_capacity(capacity_bytes);

        // header
        buffer.extend_from_slice(0x4e7f8a9d9e0f1a2bu64.to_be_bytes().as_slice());
        buffer.extend_from_slice(&(self.old_text.len() as u64).to_be_bytes());
        buffer.extend_from_slice(&(self.new_text.len() as u64).to_be_bytes());
        buffer.extend_from_slice(0u64.to_be_bytes().as_slice());

        // body
        buffer.extend_from_slice(&self.old_text);
        buffer.extend_from_slice(&self.new_text);
        Ok(buffer)
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, DiffDesError>
    where
        Self: Sized,
    {
        if bytes.len() < 32 {
            return Err(DiffDesError {
                msg: "Buffer is too small".to_string(),
            });
        }
        let mut cursor = Cursor::new(bytes);
        let mut buffer = [0u8; 8];

        // header
        cursor.read_exact(&mut buffer).unwrap();
        let magic_number = u64::from_be_bytes(buffer);
        if magic_number != 0x4e7f8a9d9e0f1a2bu64 {
            return Err(DiffDesError {
                msg: "Invalid magic number".to_string(),
            });
        }
        cursor.read_exact(&mut buffer).unwrap();
        let old_len = u64::from_be_bytes(buffer) as usize;
        cursor.read_exact(&mut buffer).unwrap();
        let new_len = u64::from_be_bytes(buffer) as usize;
        cursor.read_exact(&mut buffer).unwrap();
        let _replaces_len = u64::from_be_bytes(buffer) as usize;

        // body
        let mut diff = Self::new();
        diff.old_text.resize(old_len, 0);
        diff.new_text.resize(new_len, 0);
        cursor.read_exact(&mut diff.old_text).unwrap();
        cursor.read_exact(&mut diff.new_text).unwrap();

        Ok(diff)
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
            let merged_diff = BlobDiff::from_squash(&diff_v01, &diff_v12);
            let patched_v0 = merged_diff.patch(&v0);
            let reverted_v2 = merged_diff.revert(&v2);
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
