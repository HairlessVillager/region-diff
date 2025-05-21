use super::{Object, ObjectDeserializieError, ObjectHash};
use crate::diff::{BlobDiff, MyersDiff, RegionDiff, myers::Replace};
use std::io::{Cursor, Read};

impl Object for BlobDiff {
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

    fn deserialize(bytes: &[u8]) -> Result<Self, ObjectDeserializieError>
    where
        Self: Sized,
    {
        if bytes.len() < 32 {
            return Err(ObjectDeserializieError {
                msg: "Buffer is too small".to_string(),
            });
        }
        let mut cursor = Cursor::new(bytes);
        let mut buffer = [0u8; 8];

        // header
        cursor.read_exact(&mut buffer).unwrap();
        let magic_number = u64::from_be_bytes(buffer);
        if magic_number != 0x4e7f8a9d9e0f1a2bu64 {
            return Err(ObjectDeserializieError {
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
        let mut diff = Self {
            old_text: Vec::new(),
            new_text: Vec::new(),
        };
        diff.old_text.resize(old_len, 0);
        diff.new_text.resize(new_len, 0);
        cursor.read_exact(&mut diff.old_text).unwrap();
        cursor.read_exact(&mut diff.new_text).unwrap();

        Ok(diff)
    }

    fn hash(&self) -> ObjectHash {
        todo!()
    }
}
impl Object for MyersDiff {
    fn serialize(&self) -> Result<Vec<u8>, ()> {
        let capacity_bytes =
            32 + self.old_text.len() + self.new_text.len() + self.replaces.len() * 32;
        let mut buffer: Vec<u8> = Vec::with_capacity(capacity_bytes);

        // header
        buffer.extend_from_slice(0x4e7f8a9d9e0f1a2bu64.to_be_bytes().as_slice());
        buffer.extend_from_slice(&(self.old_text.len() as u64).to_be_bytes());
        buffer.extend_from_slice(&(self.new_text.len() as u64).to_be_bytes());
        buffer.extend_from_slice(&(self.replaces.len() as u64).to_be_bytes());

        // body
        buffer.extend_from_slice(&self.old_text);
        buffer.extend_from_slice(&self.new_text);
        for replace in &self.replaces {
            buffer.extend_from_slice(&(replace.old_idx as u64).to_be_bytes());
            buffer.extend_from_slice(&(replace.old_len as u64).to_be_bytes());
            buffer.extend_from_slice(&(replace.new_idx as u64).to_be_bytes());
            buffer.extend_from_slice(&(replace.new_len as u64).to_be_bytes());
        }
        Ok(buffer)
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, ObjectDeserializieError>
    where
        Self: Sized,
    {
        if bytes.len() < 32 {
            return Err(ObjectDeserializieError {
                msg: "Buffer is too small".to_string(),
            });
        }
        let mut cursor = Cursor::new(bytes);
        let mut buffer = [0u8; 8];

        // header
        cursor.read_exact(&mut buffer).unwrap();
        let magic_number = u64::from_be_bytes(buffer);
        if magic_number != 0x4e7f8a9d9e0f1a2bu64 {
            return Err(ObjectDeserializieError {
                msg: "Invalid magic number".to_string(),
            });
        }
        cursor.read_exact(&mut buffer).unwrap();
        let old_len = u64::from_be_bytes(buffer) as usize;
        cursor.read_exact(&mut buffer).unwrap();
        let new_len = u64::from_be_bytes(buffer) as usize;
        cursor.read_exact(&mut buffer).unwrap();
        let replaces_len = u64::from_be_bytes(buffer) as usize;

        // body
        let mut diff = Self {
            old_text: Vec::new(),
            new_text: Vec::new(),
            replaces: Vec::new(),
        };
        diff.old_text.resize(old_len, 0);
        diff.new_text.resize(new_len, 0);
        cursor.read_exact(&mut diff.old_text).unwrap();
        cursor.read_exact(&mut diff.new_text).unwrap();

        for _ in 0..replaces_len {
            cursor.read_exact(&mut buffer).unwrap();
            let old_idx = u64::from_be_bytes(buffer) as usize;
            cursor.read_exact(&mut buffer).unwrap();
            let old_len = u64::from_be_bytes(buffer) as usize;
            cursor.read_exact(&mut buffer).unwrap();
            let new_idx = u64::from_be_bytes(buffer) as usize;
            cursor.read_exact(&mut buffer).unwrap();
            let new_len = u64::from_be_bytes(buffer) as usize;
            diff.replaces.push(Replace {
                old_idx,
                old_len,
                new_idx,
                new_len,
            });
        }
        Ok(diff)
    }
    fn hash(&self) -> ObjectHash {
        todo!()
    }
}
impl Object for RegionDiff {
    fn serialize(&self) -> Result<Vec<u8>, ()> {
        todo!()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, ObjectDeserializieError>
    where
        Self: Sized,
    {
        todo!()
    }

    fn hash(&self) -> ObjectHash {
        todo!()
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
    mod test_serialize_deserialize {
        use crate::diff::Diff;

        use super::*;

        #[test]
        fn test_blob_diff() {
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
        #[test]
        fn test_myers_diff() {
            let mut old_iter = create_test_bytes(114514);
            let mut new_iter = create_test_bytes(1919810);
            for _ in 0..100_000 {
                let old = old_iter.next().unwrap();
                let new = new_iter.next().unwrap();
                let diff = MyersDiff::from_compare(&old, &new);
                let serialized = diff.serialize().unwrap();
                let deserialized = MyersDiff::deserialize(&serialized).unwrap();
                assert_eq!(diff.old_text, deserialized.old_text);
                assert_eq!(diff.new_text, deserialized.new_text);
                assert_eq!(diff.replaces, deserialized.replaces);
            }
        }
    }
}
