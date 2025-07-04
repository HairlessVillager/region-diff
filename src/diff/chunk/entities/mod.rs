use bincode::{Decode, Encode};
use fastnbt::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::diff::Diff;
use crate::diff::base::{BlobDiff, MyersDiff};
use crate::util::nbt_serde::{de, ser};

type Uuid = [i32; 4];

#[derive(Debug, Clone, Encode, Decode)]
enum EntityDiff {
    Create(BlobDiff),
    Delete(BlobDiff),
    Update(MyersDiff),
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct EntitiesDiff {
    old_uuid_list: Vec<Uuid>,
    new_uuid_list: Vec<Uuid>,
    map: BTreeMap<Uuid, EntityDiff>,
}

fn build_es_uuid_map_and_uuid_list(es: &Value) -> (BTreeMap<Uuid, &Value>, Vec<Uuid>) {
    match es {
        Value::List(es) => {
            let i = es.iter().map(|e| match e {
                Value::Compound(kv) => {
                    let uuid = match kv.get("UUID") {
                        Some(Value::IntArray(int_array)) => {
                            if int_array.len() != 4 {
                                panic!("The length of the IntArray should be 4 to form a Uuid.");
                            }
                            let mut uuid_array = [0; 4];
                            for (i, &val) in int_array.iter().enumerate() {
                                uuid_array[i] = val;
                            }
                            uuid_array
                        }
                        _ => panic!("The value for 'UUID' should be a Value::IntArray."),
                    };
                    (uuid, e)
                }
                _ => panic!("'be.x' should be Value::Compound"),
            });
            (
                BTreeMap::from_iter(i.clone().map(|(uuid, e)| (uuid, e))),
                Vec::from_iter(i.map(|(uuid, _)| uuid)),
            )
        }
        _ => panic!("'bes' should be Value::List"),
    }
}

fn build_es_map(es: &Value) -> BTreeMap<Uuid, Value> {
    match es {
        Value::List(es) => BTreeMap::from_iter(es.iter().map(|e| match e {
            Value::Compound(kv) => {
                let uuid = match kv.get("UUID") {
                    Some(Value::IntArray(int_array)) => {
                        if int_array.len() != 4 {
                            panic!("The length of the IntArray should be 4 to form a Uuid.");
                        }
                        let mut uuid_array = [0; 4];
                        for (i, &val) in int_array.iter().enumerate() {
                            uuid_array[i] = val;
                        }
                        uuid_array
                    }
                    _ => panic!("The value for 'UUID' should be a Value::IntArray."),
                };
                (uuid, e.clone())
            }
            _ => panic!("'be.x' should be Value::Compound"),
        })),
        _ => panic!("'bes' should be Value::List"),
    }
}

fn build_es_value(mut map: BTreeMap<Uuid, Value>, uuid_list: &Vec<Uuid>) -> Value {
    Value::List(Vec::from_iter(
        uuid_list.iter().map(|uuid| map.remove(uuid).unwrap()),
    ))
}

impl Diff<Value> for EntitiesDiff {
    fn from_compare(old: &Value, new: &Value) -> Self
    where
        Self: Sized,
    {
        let (old_es_map, old_uuid_list) = build_es_uuid_map_and_uuid_list(old);
        let (new_es_map, new_uuid_list) = build_es_uuid_map_and_uuid_list(new);
        let uuids = BTreeSet::from_iter(
            old_es_map
                .keys()
                .into_iter()
                .chain(new_es_map.keys().into_iter()),
        );
        let map = BTreeMap::from_iter(uuids.into_iter().map(|uuid| {
            let old = old_es_map.get(uuid);
            let new = new_es_map.get(uuid);
            let diff = match (old, new) {
                (None, Some(new)) => EntityDiff::Create(BlobDiff::from_create(&ser(new))),
                (Some(old), None) => EntityDiff::Delete(BlobDiff::from_delete(&ser(old))),
                (Some(old), Some(new)) => {
                    EntityDiff::Update(MyersDiff::from_compare(&ser(old), &ser(new)))
                }
                _ => unreachable!("Entity not exists in both old and new entities"),
            };
            (uuid.clone(), diff)
        }));
        Self {
            old_uuid_list,
            new_uuid_list,
            map,
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        let uuids = BTreeSet::from_iter(
            base.map
                .keys()
                .into_iter()
                .chain(squashing.map.keys().into_iter()),
        );
        let map =
            BTreeMap::from_iter(
                uuids.into_iter().filter_map(|uuid| {
                    let base_diff = base.map.get(uuid);
                    let squashing_diff = squashing.map.get(uuid);

                    let squashed = match (base_diff, squashing_diff) {
                        (None, None) => {
                            unreachable!(
                                "Entity with uuid={uuid:?} not exists in both base and squash",
                            )
                        }
                        (None, Some(squashing_diff)) => Some(squashing_diff.clone()),
                        (Some(base_diff), None) => Some(base_diff.clone()),
                        (Some(base_diff), Some(squashing_diff)) => {
                            match (base_diff, squashing_diff) {
                                (EntityDiff::Create(_), EntityDiff::Delete(_)) => None,
                                (EntityDiff::Delete(base), EntityDiff::Create(squashing)) => {
                                    Some(EntityDiff::Update(MyersDiff::from_compare(
                                        base.get_old_text(),
                                        squashing.get_new_text(),
                                    )))
                                }
                                (EntityDiff::Create(blob), EntityDiff::Update(myers)) => {
                                    Some(EntityDiff::Create(BlobDiff::from_create(
                                        &myers.patch(blob.get_new_text()),
                                    )))
                                }
                                (EntityDiff::Update(myers), EntityDiff::Delete(blob)) => {
                                    Some(EntityDiff::Delete(BlobDiff::from_delete(
                                        &myers.revert(blob.get_old_text()),
                                    )))
                                }
                                (EntityDiff::Update(base), EntityDiff::Update(squashing)) => Some(
                                    EntityDiff::Update(MyersDiff::from_squash(base, squashing)),
                                ),
                                _ => unreachable!("Mismatched base diff and squashing diff"),
                            }
                        }
                    };
                    squashed.map(|diff| (uuid.clone(), diff))
                }),
            );
        Self {
            old_uuid_list: base.old_uuid_list.clone(),
            new_uuid_list: squashing.new_uuid_list.clone(),
            map,
        }
    }

    fn patch(&self, old: &Value) -> Value {
        let mut es_map = build_es_map(old);
        for (uuid, diff) in self.map.iter() {
            let old_e = es_map.get(uuid);
            let new_e = match (old_e, diff) {
                (None, EntityDiff::Create(diff)) => Some(de(&diff.patch0())),
                (Some(_), EntityDiff::Delete(_)) => None,
                (Some(old), EntityDiff::Update(diff)) => Some(de(&diff.patch(&ser(old)))),
                (old_e, diff) => unreachable!("{:?} and {:?}", old_e, diff),
            };
            match new_e {
                Some(e) => es_map.insert(*uuid, e),
                None => es_map.remove(uuid),
            };
        }
        build_es_value(es_map, &self.new_uuid_list)
    }

    fn revert(&self, new: &Value) -> Value {
        let mut es_map = build_es_map(new);
        for (uuid, diff) in self.map.iter() {
            let new_e = es_map.get(uuid);
            let old_e = match (diff, new_e) {
                (EntityDiff::Create(_), Some(_)) => None,
                (EntityDiff::Delete(diff), None) => Some(de(&diff.revert0())),
                (EntityDiff::Update(diff), Some(new)) => Some(de(&diff.revert(&ser(new)))),
                (dif, new_e) => unreachable!("{:?} and {:?}", dif, new_e),
            };
            match old_e {
                Some(e) => es_map.insert(*uuid, e),
                None => es_map.remove(uuid),
            };
        }
        build_es_value(es_map, &self.old_uuid_list)
    }
}

#[derive(Debug, Encode, Decode, Clone)]
pub struct EntitiesChunkDiff {
    entities: EntitiesDiff,
    others: MyersDiff,
}

static ERR_MSG_OLD: &str = "Invalid old nbt";
static ERR_MSG_NEW: &str = "Invalid new nbt";

impl Diff<Value> for EntitiesChunkDiff {
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
        let diff_entities;
        {
            let old_entities = old.remove("Entities").unwrap();
            let new_entities = new.remove("Entities").unwrap();
            diff_entities = EntitiesDiff::from_compare(&old_entities, &new_entities);
        }

        let diff_others;
        {
            let old_others = ser(&Value::Compound(old.clone()));
            let new_others = ser(&Value::Compound(new.clone()));
            diff_others = MyersDiff::from_compare(&old_others, &new_others);
        }

        Self {
            entities: diff_entities,
            others: diff_others,
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        let entities = EntitiesDiff::from_squash(&base.entities, &squashing.entities);
        let others = MyersDiff::from_squash(&base.others, &squashing.others);
        Self { entities, others }
    }

    fn patch(&self, old: &Value) -> Value {
        let mut old = match old {
            Value::Compound(x) => x.clone(),
            _ => panic!("{}", ERR_MSG_OLD),
        };
        let entities;
        {
            let old_entities = old.remove("Entities").unwrap();
            entities = self.entities.patch(&old_entities);
        }
        let mut others;
        {
            let old_others = ser(&Value::Compound(old.clone()));
            let new_others = self.others.patch(&old_others);
            let wrapped_others: Value = de(&new_others);
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("{}", ERR_MSG_NEW),
            }
        }

        others.insert("Entities".to_string(), entities);

        Value::Compound(others)
    }

    fn revert(&self, new: &Value) -> Value {
        let mut new = match new {
            Value::Compound(x) => x.clone(),
            _ => panic!("{}", ERR_MSG_OLD),
        };

        let entities;
        {
            let new_entities = new.remove("Entities").unwrap();
            entities = self.entities.revert(&new_entities);
        }

        let mut others;
        {
            let new_others = ser(&Value::Compound(new.clone()));
            let old_others = self.others.revert(&new_others);
            let wrapped_others: Value = de(&old_others);
            others = match wrapped_others {
                Value::Compound(x) => x,
                _ => panic!("{}", ERR_MSG_NEW),
            };
        }
        others.insert("Entities".to_string(), entities);
        Value::Compound(others)
    }
}

#[cfg(test)]
mod tests {
    mod test_in_continuous_data {
        use crate::diff::Diff;
        use crate::diff::chunk::EntitiesChunkDiff;
        use crate::util::nbt_serde::de;
        use crate::util::test::get_test_chunk;
        use rand::prelude::StdRng;
        use rand::{RngCore, SeedableRng};
        use std::path::PathBuf;

        #[test]
        fn test_diff_patch_revert() -> () {
            let mut rng_old = StdRng::seed_from_u64(114514);
            let mut rng_new = rng_old.clone();
            rng_new.next_u32();
            let binding = PathBuf::from(
                "./resources/test-payload/entities/mca/hairlessvillager-0/r.0.0v0.mca",
            );
            let mut old_iter = get_test_chunk(&binding, &mut rng_old);
            let binding = PathBuf::from(
                "./resources/test-payload/entities/mca/hairlessvillager-0/r.0.0v1.mca",
            );
            let mut new_iter = get_test_chunk(&binding, &mut rng_new);
            for _ in 0..50 {
                let old = de(&old_iter.next().unwrap());
                let new = de(&new_iter.next().unwrap());
                let diff = EntitiesChunkDiff::from_compare(&old, &new);
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
                "./resources/test-payload/entities/mca/hairlessvillager-0/r.0.0v1.mca",
            );
            let mut v0_iter = get_test_chunk(&binding, &mut rng_v0);
            let binding = PathBuf::from(
                "./resources/test-payload/entities/mca/hairlessvillager-0/r.0.0v1.mca",
            );
            let mut v1_iter = get_test_chunk(&binding, &mut rng_v1);
            let binding = PathBuf::from(
                "./resources/test-payload/entities/mca/hairlessvillager-0/r.0.0v2.mca",
            );
            let mut v2_iter = get_test_chunk(&binding, &mut rng_v2);
            for _ in 0..50 {
                let v0 = de(&v0_iter.next().unwrap());
                let v1 = de(&v1_iter.next().unwrap());
                let v2 = de(&v2_iter.next().unwrap());
                let diff_v01 = EntitiesChunkDiff::from_compare(&v0, &v1);
                let diff_v12 = EntitiesChunkDiff::from_compare(&v1, &v2);
                let squashed_diff = EntitiesChunkDiff::from_squash(&diff_v01, &diff_v12);
                let patched_v0 = squashed_diff.patch(&v0);
                let reverted_v2 = squashed_diff.revert(&v2);
                assert_eq!(v2, patched_v0);
                assert_eq!(v0, reverted_v2);
            }
        }
    }
}
