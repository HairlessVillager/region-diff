use bincode::{Decode, Encode};
use fastnbt::Value;

use crate::{
    diff::{Diff, base::MyersDiff, nbt::BlockEntitiesDiff},
    util::nbt_serde::{de, ser},
};

#[derive(Debug, Encode, Decode, Clone)]
pub struct ChunkDiff {
    block_entities: BlockEntitiesDiff,
    sections: Vec<MyersDiff>,
    others: MyersDiff,
}

static ERR_MSG_OLD: &str = "Invalid old nbt";
static ERR_MSG_NEW: &str = "Invalid new nbt";

impl Diff<Value> for ChunkDiff {
    fn from_compare(old: &Value, new: &Value) -> Self
    where
        Self: Sized,
    {
        let mut old = match old {
            Value::Compound(x) => x.clone(),
            _ => panic!("{}", ERR_MSG_OLD),
        };
        let mut new = match new {
            Value::Compound(x) => x.clone(),
            _ => panic!("{}", ERR_MSG_NEW),
        };

        let diff_block_entities;
        {
            let old_block_entities = old.remove("block_entities").expect(ERR_MSG_OLD);
            let new_block_entities = new.remove("block_entities").expect(ERR_MSG_NEW);
            diff_block_entities =
                BlockEntitiesDiff::from_compare(&old_block_entities, &new_block_entities);
        }

        let diff_sections;
        {
            let old_sections = old.remove("sections").expect(ERR_MSG_OLD);
            let old_sections = match old_sections {
                Value::List(x) => x,
                _ => panic!("{}", ERR_MSG_OLD),
            };
            let new_sections = new.remove("sections").expect(ERR_MSG_NEW);
            let new_sections = match new_sections {
                Value::List(x) => x,
                _ => panic!("{}", ERR_MSG_NEW),
            };
            assert_eq!(old_sections.len(), new_sections.len());

            let mut mut_diff_sections = Vec::with_capacity(old_sections.len());
            for (old, new) in old_sections.iter().zip(new_sections.iter()) {
                let old = ser(old);
                let new = ser(new);
                let diff = MyersDiff::from_compare(&old, &new);
                mut_diff_sections.push(diff);
            }
            diff_sections = mut_diff_sections;
        }

        let diff_others;
        {
            let old_others = ser(&Value::Compound(old.clone()));
            let new_others = ser(&Value::Compound(new.clone()));
            diff_others = MyersDiff::from_compare(&old_others, &new_others);
        }

        Self {
            block_entities: diff_block_entities,
            sections: diff_sections,
            others: diff_others,
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        let block_entities =
            BlockEntitiesDiff::from_squash(&base.block_entities, &squashing.block_entities);
        let sections = base
            .sections
            .iter()
            .zip(squashing.sections.iter())
            .map(|(base, squashing)| MyersDiff::from_squash(base, squashing))
            .collect();
        let others = MyersDiff::from_squash(&base.others, &squashing.others);
        Self {
            block_entities,
            sections,
            others,
        }
    }

    fn patch(&self, old: &Value) -> Value {
        let mut old = match old {
            Value::Compound(x) => x.clone(),
            _ => panic!("{}", ERR_MSG_OLD),
        };

        let block_entities;
        {
            let old_block_entities = old.remove("block_entities").expect(ERR_MSG_OLD);
            block_entities = self.block_entities.patch(&old_block_entities);
        }

        let sections: Vec<Value>;
        {
            let old_sections = old.remove("sections").expect(ERR_MSG_OLD);
            let old_sections = match old_sections {
                Value::List(x) => x,
                _ => panic!("{}", ERR_MSG_OLD),
            };
            sections = old_sections
                .iter()
                .zip(self.sections.iter())
                .map(|(old, diff)| {
                    let old = ser(old);
                    let new = diff.patch(&old);
                    let new = de(&new);
                    new
                })
                .collect()
        }

        let mut others;
        {
            let old_others = ser(&Value::Compound(old));
            let new_others = self.others.patch(&old_others);
            let wrapped_others: Value = de(&new_others);
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("{}", ERR_MSG_NEW),
            }
        }

        others.insert("sections".to_string(), Value::List(sections));
        others.insert("block_entities".to_string(), block_entities);

        Value::Compound(others)
    }

    fn revert(&self, new: &Value) -> Value {
        let mut new = match new {
            Value::Compound(x) => x.clone(),
            _ => panic!("{}", ERR_MSG_NEW),
        };

        let block_entities;
        {
            let new_block_entities = new.remove("block_entities").expect(ERR_MSG_NEW);
            block_entities = self.block_entities.revert(&new_block_entities);
        }

        let sections: Vec<Value>;
        {
            let new_sections = new.remove("sections").expect(ERR_MSG_NEW);
            let new_sections = match new_sections {
                Value::List(x) => x,
                _ => panic!("{}", ERR_MSG_NEW),
            };
            sections = new_sections
                .iter()
                .zip(self.sections.iter())
                .map(|(new_section, diff)| {
                    let new_bytes = ser(new_section);
                    let old_bytes = diff.revert(&new_bytes);
                    de(&old_bytes)
                })
                .collect();
        }

        let mut others;
        {
            let new_others = ser(&Value::Compound(new));
            let old_others = self.others.revert(&new_others);
            let wrapped_others: Value = de(&old_others);
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("{}", ERR_MSG_OLD),
            };
        }

        others.insert("sections".to_string(), Value::List(sections));
        others.insert("block_entities".to_string(), block_entities);

        Value::Compound(others)
    }
}
#[cfg(test)]
mod tests {
    use rand::prelude::*;

    use super::*;
    mod test_in_continuous_data {
        use std::path::PathBuf;

        use crate::util::test::get_test_chunk;

        use super::*;
        #[test]
        fn test_diff_patch_revert() -> () {
            let mut rng_old = StdRng::seed_from_u64(114514);
            let mut rng_new = rng_old.clone();
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250511.mca",
            );
            let mut old_iter = get_test_chunk(&binding, &mut rng_old);
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250516.mca",
            );
            let mut new_iter = get_test_chunk(&binding, &mut rng_new);
            for _ in 0..50 {
                let old = de(&old_iter.next().unwrap());
                let new = de(&new_iter.next().unwrap());
                let diff = ChunkDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);
                assert_eq!(new, patched_old);
                assert_eq!(old, reverted_new);
            }
        }
        #[test]
        fn test_diff_squash() -> () {
            let mut rng_v0 = StdRng::seed_from_u64(114514);
            let mut rng_v1 = rng_v0.clone();
            let mut rng_v2 = rng_v1.clone();
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250511.mca",
            );
            let mut v0_iter = get_test_chunk(&binding, &mut rng_v0);
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250513.mca",
            );
            let mut v1_iter = get_test_chunk(&binding, &mut rng_v1);
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250515.mca",
            );
            let mut v2_iter = get_test_chunk(&binding, &mut rng_v2);
            for _ in 0..50 {
                let v0 = de(&v0_iter.next().unwrap());
                let v1 = de(&v1_iter.next().unwrap());
                let v2 = de(&v2_iter.next().unwrap());
                let diff_v01 = ChunkDiff::from_compare(&v0, &v1);
                let diff_v12 = ChunkDiff::from_compare(&v1, &v2);
                let squashed_diff = ChunkDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);
                assert_eq!(v2, patched_v0);
                assert_eq!(v0, reverted_v2);
            }
        }
    }
    mod test_in_noncontinuous_data {
        use std::path::PathBuf;

        use crate::util::test::get_test_chunk;

        use super::*;
        #[test]
        fn test_diff_patch_revert() -> () {
            let mut rng_old = StdRng::seed_from_u64(114514);
            let mut rng_new = rng_old.clone();
            rng_new.next_u32();
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250511.mca",
            );
            let mut old_iter = get_test_chunk(&binding, &mut rng_old);
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250516.mca",
            );
            let mut new_iter = get_test_chunk(&binding, &mut rng_new);
            for _ in 0..10 {
                let old = de(&old_iter.next().unwrap());
                let new = de(&new_iter.next().unwrap());
                let diff = ChunkDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);
                assert_eq!(new, patched_old);
                assert_eq!(old, reverted_new);
            }
        }
        #[test]
        fn test_diff_squash() -> () {
            let mut rng_v0 = StdRng::seed_from_u64(114514);
            let mut rng_v1 = rng_v0.clone();
            rng_v1.next_u32();
            let mut rng_v2 = rng_v1.clone();
            rng_v2.next_u32();
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250511.mca",
            );
            let mut v0_iter = get_test_chunk(&binding, &mut rng_v0);
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250513.mca",
            );
            let mut v1_iter = get_test_chunk(&binding, &mut rng_v1);
            let binding = PathBuf::from(
                "./resources/test-payload/region/mca/hairlessvillager-0/20250515.mca",
            );
            let mut v2_iter = get_test_chunk(&binding, &mut rng_v2);
            for _ in 0..10 {
                let v0 = de(&v0_iter.next().unwrap());
                let v1 = de(&v1_iter.next().unwrap());
                let v2 = de(&v2_iter.next().unwrap());
                let diff_v01 = ChunkDiff::from_compare(&v0, &v1);
                let diff_v12 = ChunkDiff::from_compare(&v1, &v2);
                let squashed_diff = ChunkDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);
                assert_eq!(v2, patched_v0);
                assert_eq!(v0, reverted_v2);
            }
        }
    }
}
