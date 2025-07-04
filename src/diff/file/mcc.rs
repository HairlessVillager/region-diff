use bincode::{Decode, Encode};
use fastnbt::Value;

use crate::{
    compress::CompressionType,
    diff::{Diff, base::BlobDiff},
    util::nbt_serde::{de, ser},
};

#[derive(Debug, Clone, Encode, Decode)]
pub enum MCCDiff<D>
where
    D: Diff<Value>,
{
    Create(BlobDiff),
    Delete(BlobDiff),
    Update(D),
}

impl<D> Diff<Vec<u8>> for MCCDiff<D>
where
    D: Diff<Value> + bincode::Decode<MCCDiff<D>>,
{
    fn from_compare(old: &Vec<u8>, new: &Vec<u8>) -> Self
    where
        Self: Sized,
    {
        match (old.is_empty(), new.is_empty()) {
            (true, true) => panic!("Cannot compare two empty MCC files"),
            (true, false) => {
                // Create
                let decompressed_new = CompressionType::Zlib
                    .decompress_all(new)
                    .expect("Failed to decompress new MCC file for create");
                Self::Create(BlobDiff::from_create(&decompressed_new))
            }
            (false, true) => {
                // Delete
                let decompressed_old = CompressionType::Zlib
                    .decompress_all(old)
                    .expect("Failed to decompress old MCC file for delete");
                Self::Delete(BlobDiff::from_delete(&decompressed_old))
            }
            (false, false) => {
                // Update
                let old_nbt: Value = de(&CompressionType::Zlib
                    .decompress_all(old)
                    .expect("Failed to decompress old MCC file for update"));
                let new_nbt: Value = de(&CompressionType::Zlib
                    .decompress_all(new)
                    .expect("Failed to decompress new MCC file for update"));
                Self::Update(D::from_compare(&old_nbt, &new_nbt))
            }
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        match (base, squashing) {
            // Create -> Update => Create
            (Self::Create(base_blob), Self::Update(squashing_chunk)) => {
                let base_nbt = de(&base_blob.patch0());
                let squashed_nbt = squashing_chunk.patch(&base_nbt);
                Self::Create(BlobDiff::from_create(&ser(&squashed_nbt)))
            }
            // Create -> Delete => No Diff (panic because it shouldn't happen in practice)
            (Self::Create(_), Self::Delete(_)) => {
                panic!(
                    "Squashing a Create then Delete diff results in no change, which is illogical for a single file diff."
                )
            }
            // Update -> Update => Update
            (Self::Update(base_chunk), Self::Update(squashing_chunk)) => {
                Self::Update(D::from_squash(base_chunk, squashing_chunk))
            }
            // Update -> Delete => Delete
            (Self::Update(base_chunk), Self::Delete(squashing_blob)) => {
                let squashing_nbt = de(&squashing_blob.revert0());
                let base_nbt = base_chunk.revert(&squashing_nbt);
                Self::Delete(BlobDiff::from_delete(&ser(&base_nbt)))
            }
            // Delete -> Create => Update
            (Self::Delete(base_blob), Self::Create(squashing_blob)) => {
                let old_nbt = de(&base_blob.revert0());
                let new_nbt = de(&squashing_blob.patch0());
                Self::Update(D::from_compare(&old_nbt, &new_nbt))
            }
            _ => panic!("Invalid squash combination for MCCDiff"),
        }
    }

    fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
        let patched_nbt = match self {
            Self::Create(blob_diff) => {
                // `old` should be empty
                if !old.is_empty() {
                    panic!("Cannot apply a Create diff to a non-empty file");
                }
                de(&blob_diff.patch(old))
            }
            Self::Delete(_) => {
                // Result is an empty file, but we represent it as empty byte vector
                return Vec::new();
            }
            Self::Update(chunk_diff) => {
                let old_nbt: Value = de(&CompressionType::Zlib
                    .decompress_all(old)
                    .expect("Failed to decompress old MCC file for patch"));
                chunk_diff.patch(&old_nbt)
            }
        };
        CompressionType::Zlib
            .compress_all(&ser(&patched_nbt))
            .expect("Failed to compress patched NBT")
    }

    fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
        let reverted_nbt = match self {
            Self::Create(_) => {
                // Result is an empty file
                return Vec::new();
            }
            Self::Delete(blob_diff) => {
                // `new` should be empty
                if !new.is_empty() {
                    panic!("Cannot apply a Delete diff to a non-empty file");
                }
                de(&blob_diff.revert(new))
            }
            Self::Update(chunk_diff) => {
                let new_nbt: Value = de(&CompressionType::Zlib
                    .decompress_all(new)
                    .expect("Failed to decompress new MCC file for revert"));
                chunk_diff.revert(&new_nbt)
            }
        };
        CompressionType::Zlib
            .compress_all(&ser(&reverted_nbt))
            .expect("Failed to compress reverted NBT")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, LogConfig, with_test_config};
    use crate::diff::chunk::RegionChunkDiff;
    use crate::util::test::assert_mcc_eq;
    use std::fs;

    static TEST_CONFIG: Config = Config {
        log_config: LogConfig::NoLog,
        threads: 16,
    };

    fn read_mcc_file(version: &str) -> Vec<u8> {
        let path = format!(
            "./resources/test-payload/region/mcc/ycc-small-data/c.0.0{}.mcc",
            version
        );
        fs::read(path).unwrap()
    }

    #[test]
    fn test_diff_patch_revert() {
        with_test_config(TEST_CONFIG.clone(), || {
            let v1 = read_mcc_file("v1");
            let v2 = read_mcc_file("v2");
            let diff = MCCDiff::<RegionChunkDiff>::from_compare(&v1, &v2);
            let patched_v1 = diff.patch(&v1);
            let reverted_v2 = diff.revert(&v2);
            assert_mcc_eq(patched_v1, v2);
            assert_mcc_eq(reverted_v2, v1);
        });
    }

    #[test]
    fn test_diff_squash() {
        with_test_config(TEST_CONFIG.clone(), || {
            let v1 = read_mcc_file("v1");
            let v2 = read_mcc_file("v2");
            let v3 = read_mcc_file("v3");
            let diff_v1_v2 = MCCDiff::<RegionChunkDiff>::from_compare(&v1, &v2);
            let diff_v2_v3 = MCCDiff::<RegionChunkDiff>::from_compare(&v2, &v3);
            let squashed_diff = MCCDiff::<RegionChunkDiff>::from_squash(&diff_v1_v2, &diff_v2_v3);
            let patched_v1 = squashed_diff.patch(&v1);
            let reverted_v3 = squashed_diff.revert(&v3);
            assert_mcc_eq(patched_v1, v3);
            assert_mcc_eq(reverted_v3, v1);
        });
    }
}
