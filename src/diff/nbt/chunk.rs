use bincode::{Decode, Encode, decode_from_slice, encode_to_vec};
use fastnbt::Value;
use std::collections::BTreeMap;

use crate::{
    diff::{Diff, base::MyersDiff},
    object::{Serde, SerdeError},
    util::create_bincode_config,
};

#[derive(Debug, Encode, Decode, Clone)]
pub struct ChunkDiff {
    block_entities: MyersDiff,
    sections: Vec<MyersDiff>,
    others: MyersDiff,
}
fn wrap_with_root_compound(value: Value) -> Value {
    Value::Compound(BTreeMap::from([("root".to_string(), value)]))
}
fn unwrap_with_root_compound(value: Value) -> Value {
    match value {
        Value::Compound(mut map) => map.remove("root").unwrap(),
        _ => panic!("root compound not exists"),
    }
}
fn pop_block_entities_from(compound_map: &mut BTreeMap<String, Value>) -> Vec<u8> {
    let block_entities = compound_map.remove("block_entities").unwrap();
    let wrapped = wrap_with_root_compound(block_entities);
    fastnbt::to_bytes(&wrapped).unwrap()
}
impl Diff for ChunkDiff {
    fn from_compare(old_nbt: &[u8], new_nbt: &[u8]) -> Self
    where
        Self: Sized,
    {
        let old: Value = fastnbt::from_bytes(old_nbt).unwrap();
        let mut old = match old {
            Value::Compound(x) => x,
            _ => panic!("invalid old nbt"),
        };
        let new: Value = fastnbt::from_bytes(new_nbt).unwrap();
        let mut new = match new {
            Value::Compound(x) => x,
            _ => panic!("invalid new nbt"),
        };

        let diff_block_entities;
        {
            let old_block_entities = pop_block_entities_from(&mut old);
            let new_block_entities = pop_block_entities_from(&mut new);
            diff_block_entities = MyersDiff::from_compare(&old_block_entities, &new_block_entities);
        }

        let diff_sections;
        {
            let old_sections = old.remove("sections").unwrap();
            let old_sections = match old_sections {
                Value::List(x) => x,
                _ => panic!("invalid old nbt"),
            };
            let new_sections = new.remove("sections").unwrap();
            let new_sections = match new_sections {
                Value::List(x) => x,
                _ => panic!("invalid new nbt"),
            };
            assert_eq!(old_sections.len(), new_sections.len());

            let mut mut_diff_sections = Vec::with_capacity(old_sections.len());
            for (old, new) in old_sections.iter().zip(new_sections.iter()) {
                let old = fastnbt::to_bytes(old).unwrap();
                let new = fastnbt::to_bytes(new).unwrap();
                let diff = MyersDiff::from_compare(&old, &new);
                mut_diff_sections.push(diff);
            }
            diff_sections = mut_diff_sections;
        }

        let diff_others;
        {
            let old_others = fastnbt::to_bytes(&Value::Compound(old)).unwrap();
            let new_others = fastnbt::to_bytes(&Value::Compound(new)).unwrap();
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
            MyersDiff::from_squash(&base.block_entities, &squashing.block_entities);
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

    fn patch(&self, old_nbt: &[u8]) -> Vec<u8> {
        let old: Value = fastnbt::from_bytes(old_nbt).unwrap();
        let mut old = match old {
            Value::Compound(x) => x,
            _ => panic!("invalid old nbt"),
        };

        let block_entities;
        {
            let old_block_entities = pop_block_entities_from(&mut old);
            let new_block_entities = self.block_entities.patch(&old_block_entities);
            let new_block_entities = fastnbt::from_bytes(&new_block_entities).unwrap();
            block_entities = unwrap_with_root_compound(new_block_entities);
        }

        let sections: Vec<Value>;
        {
            let old_sections = old.remove("sections").unwrap();
            let old_sections = match old_sections {
                Value::List(x) => x,
                _ => panic!("invalid old nbt"),
            };
            sections = old_sections
                .iter()
                .zip(self.sections.iter())
                .map(|(old, diff)| {
                    let old = fastnbt::to_bytes(old).unwrap();
                    let new = diff.patch(&old);
                    let new = fastnbt::from_bytes(&new).unwrap();
                    new
                })
                .collect()
        }

        let mut others;
        {
            let old_others = fastnbt::to_bytes(&Value::Compound(old)).unwrap();
            let new_others = self.others.patch(&old_others);
            let wrapped_others: Value = fastnbt::from_bytes(&new_others).unwrap();
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("invalid new nbt"),
            }
        }

        others.insert("sections".to_string(), Value::List(sections));
        others.insert("block_entities".to_string(), block_entities);

        fastnbt::to_bytes(&Value::Compound(others)).unwrap()
    }

    fn revert(&self, new_nbt: &[u8]) -> Vec<u8> {
        let new: Value = fastnbt::from_bytes(new_nbt).unwrap();
        let mut new = match new {
            Value::Compound(x) => x,
            _ => panic!("invalid new nbt"),
        };

        let block_entities;
        {
            let new_block_entities = pop_block_entities_from(&mut new);
            let old_block_entities = self.block_entities.revert(&new_block_entities);
            let old_block_entities = fastnbt::from_bytes(&old_block_entities).unwrap();
            block_entities = unwrap_with_root_compound(old_block_entities);
        }

        let sections: Vec<Value>;
        {
            let new_sections = new.remove("sections").unwrap();
            let new_sections = match new_sections {
                Value::List(x) => x,
                _ => panic!("invalid new nbt"),
            };
            sections = new_sections
                .iter()
                .zip(self.sections.iter())
                .map(|(new_section, diff)| {
                    let new_bytes = fastnbt::to_bytes(new_section).unwrap();
                    let old_bytes = diff.revert(&new_bytes);
                    fastnbt::from_bytes(&old_bytes).unwrap()
                })
                .collect();
        }

        let mut others;
        {
            let new_others = fastnbt::to_bytes(&Value::Compound(new)).unwrap();
            let old_others = self.others.revert(&new_others);
            let wrapped_others: Value = fastnbt::from_bytes(&old_others).unwrap();
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("invalid old nbt"),
            };
        }

        others.insert("sections".to_string(), Value::List(sections));
        others.insert("block_entities".to_string(), block_entities);

        fastnbt::to_bytes(&Value::Compound(others)).unwrap()
    }
}
impl Serde for ChunkDiff {
    fn serialize(&self) -> Result<Vec<u8>, SerdeError> {
        encode_to_vec(self, create_bincode_config()).map_err(|e| SerdeError::from(e))
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, SerdeError>
    where
        Self: Sized,
    {
        let result: Result<(ChunkDiff, usize), _> =
            decode_from_slice(bytes, create_bincode_config());
        result
            .map(|(diff, _)| diff)
            .map_err(|e| SerdeError::from(e))
    }
}
#[cfg(test)]
mod tests {
    use rand::prelude::*;

    use crate::{mca::MCAReader, util::create_chunk_ixz_iter};

    use super::*;
    fn get_test_chunk(path: &str, rng: &mut StdRng) -> impl Iterator<Item = Vec<u8>> {
        let mut reader = MCAReader::from_file(path, false).unwrap();
        let mut xzs = [(0, 0); 1024];
        for (i, x, z) in create_chunk_ixz_iter() {
            xzs[i] = (x, z);
        }
        xzs.shuffle(rng);
        xzs.into_iter()
            .map(move |(x, z)| reader.get_chunk(x, z).unwrap().unwrap().nbt.clone())
    }
    mod test_in_continuous_data {
        use fastnbt::Value;

        use super::*;
        #[test]
        fn test_diff_patch_revert() -> () {
            let mut rng_old = StdRng::seed_from_u64(114514);
            let mut rng_new = rng_old.clone();
            let mut old_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", &mut rng_old);
            let mut new_iter = get_test_chunk("./resources/mca/r.1.2.20250516.mca", &mut rng_new);
            for _ in 0..100 {
                let old = old_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                let new = new_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                let diff = ChunkDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);

                assert_eq!(
                    fastnbt::from_bytes::<Value>(&new),
                    fastnbt::from_bytes::<Value>(&patched_old)
                );
                assert_eq!(
                    fastnbt::from_bytes::<Value>(&old),
                    fastnbt::from_bytes::<Value>(&reverted_new)
                );
            }
        }
        #[test]
        fn test_diff_squash() -> () {
            let mut rng_v0 = StdRng::seed_from_u64(114514);
            let mut rng_v1 = rng_v0.clone();
            rng_v1.next_u32();
            let mut rng_v2 = rng_v1.clone();
            rng_v2.next_u32();
            let mut v0_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", &mut rng_v0);
            let mut v1_iter = get_test_chunk("./resources/mca/r.1.2.20250513.mca", &mut rng_v1);
            let mut v2_iter = get_test_chunk("./resources/mca/r.1.2.20250515.mca", &mut rng_v2);
            for _ in 0..100 {
                let v0 = v0_iter.next().unwrap();
                let v1 = v1_iter.next().unwrap();
                let v2 = v2_iter.next().unwrap();
                let diff_v01 = ChunkDiff::from_compare(&v0, &v1);
                let diff_v12 = ChunkDiff::from_compare(&v1, &v2);
                let squashed_diff = ChunkDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);

                assert_eq!(
                    fastnbt::from_bytes::<Value>(&v2),
                    fastnbt::from_bytes::<Value>(&patched_v0)
                );
                assert_eq!(
                    fastnbt::from_bytes::<Value>(&v0),
                    fastnbt::from_bytes::<Value>(&reverted_v2)
                );
            }
        }
    }
    mod test_in_noncontinuous_data {
        use fastnbt::Value;

        use super::*;
        #[test]
        fn test_diff_patch_revert() -> () {
            let mut rng_old = StdRng::seed_from_u64(114514);
            let mut rng_new = rng_old.clone();
            rng_new.next_u32();
            let mut old_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", &mut rng_old);
            let mut new_iter = get_test_chunk("./resources/mca/r.1.2.20250516.mca", &mut rng_new);
            for _ in 0..3 {
                let old = old_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                let new = new_iter.next().unwrap(); // NOTE: root compound has unsorted key, cannot assert_eq directly
                let diff = ChunkDiff::from_compare(&old, &new);
                let patched_old = diff.patch(&old);
                let reverted_new = diff.revert(&new);

                assert_eq!(
                    fastnbt::from_bytes::<Value>(&new),
                    fastnbt::from_bytes::<Value>(&patched_old)
                );
                assert_eq!(
                    fastnbt::from_bytes::<Value>(&old),
                    fastnbt::from_bytes::<Value>(&reverted_new)
                );
            }
        }
        #[test]
        fn test_diff_squash() -> () {
            let mut rng_v0 = StdRng::seed_from_u64(114514);
            let mut rng_v1 = rng_v0.clone();
            rng_v1.next_u32();
            let mut rng_v2 = rng_v1.clone();
            rng_v2.next_u32();
            let mut v0_iter = get_test_chunk("./resources/mca/r.1.2.20250511.mca", &mut rng_v0);
            let mut v1_iter = get_test_chunk("./resources/mca/r.1.2.20250513.mca", &mut rng_v1);
            let mut v2_iter = get_test_chunk("./resources/mca/r.1.2.20250515.mca", &mut rng_v2);
            for _ in 0..3 {
                let v0 = v0_iter.next().unwrap();
                let v1 = v1_iter.next().unwrap();
                let v2 = v2_iter.next().unwrap();
                let diff_v01 = ChunkDiff::from_compare(&v0, &v1);
                let diff_v12 = ChunkDiff::from_compare(&v1, &v2);
                let squashed_diff = ChunkDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);

                assert_eq!(
                    fastnbt::from_bytes::<Value>(&v2),
                    fastnbt::from_bytes::<Value>(&patched_v0)
                );
                assert_eq!(
                    fastnbt::from_bytes::<Value>(&v0),
                    fastnbt::from_bytes::<Value>(&reverted_v2)
                );
            }
        }
    }
}
