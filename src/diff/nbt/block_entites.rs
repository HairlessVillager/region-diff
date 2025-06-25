use std::collections::{BTreeMap, BTreeSet};

use bincode::{Decode, Encode};
use fastnbt::Value;

use crate::{
    diff::{
        Diff,
        base::{BlobDiff, MyersDiff},
    },
    util::{fastnbt_deserialize as de, fastnbt_serialize as ser},
};
type XYZ = (i32, i32, i32);

#[derive(Debug, Clone, Encode, Decode)]
enum BlockEntityDiff {
    Create(BlobDiff),
    Delete(BlobDiff),
    UpdateSameID(MyersDiff),
    UpdateDiffID(BlobDiff),
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct BlockEntitiesDiff {
    old_xyz_list: Vec<XYZ>,
    new_xyz_list: Vec<XYZ>,
    map: BTreeMap<XYZ, BlockEntityDiff>,
}
fn build_bes_id_map_and_xyz_list(bes: &Value) -> (BTreeMap<XYZ, (String, &Value)>, Vec<XYZ>) {
    match bes {
        Value::List(bes) => {
            let i = bes.iter().map(|be| match be {
                Value::Compound(kv) => {
                    let x = match kv.get("x").unwrap() {
                        Value::Int(i) => *i,
                        _ => panic!("be.x should be Value::Int"),
                    };
                    let y = match kv.get("y").unwrap() {
                        Value::Int(i) => *i,
                        _ => panic!("be.y should be Value::Int"),
                    };
                    let z = match kv.get("z").unwrap() {
                        Value::Int(i) => *i,
                        _ => panic!("be.z should be Value::Int"),
                    };
                    let id = match kv.get("id").unwrap() {
                        Value::String(s) => s.clone(),
                        _ => panic!("be.id should be Value::String"),
                    };
                    ((x, y, z), (id, be))
                }
                _ => panic!("be should be Value::Compound"),
            });
            (
                BTreeMap::from_iter(i.clone()),
                Vec::from_iter(i.clone().map(|(xyz, _)| xyz)),
            )
        }
        _ => panic!("bes should be Value::List"),
    }
}
fn build_bes_map(bes: &Value) -> BTreeMap<XYZ, Value> {
    match bes {
        Value::List(bes) => BTreeMap::from_iter(bes.iter().map(|be| match be {
            Value::Compound(kv) => {
                let x = match kv.get("x").unwrap() {
                    Value::Int(i) => *i,
                    _ => panic!("be.x should be Value::Int"),
                };
                let y = match kv.get("y").unwrap() {
                    Value::Int(i) => *i,
                    _ => panic!("be.y should be Value::Int"),
                };
                let z = match kv.get("z").unwrap() {
                    Value::Int(i) => *i,
                    _ => panic!("be.z should be Value::Int"),
                };
                ((x, y, z), be.clone())
            }
            _ => panic!("be should be Value::Compound"),
        })),
        _ => panic!("bes should be Value::List"),
    }
}
fn build_bes_value(mut map: BTreeMap<XYZ, Value>, xyz_list: &Vec<XYZ>) -> Value {
    Value::List(Vec::from_iter(
        xyz_list.iter().map(|k| map.remove(k).unwrap()),
    ))
}
impl Diff<Value> for BlockEntitiesDiff {
    fn from_compare(old: &Value, new: &Value) -> Self {
        let (old_bes_map, old_xyz_list) = build_bes_id_map_and_xyz_list(old);
        let (new_bes_map, new_xyz_list) = build_bes_id_map_and_xyz_list(new);
        let xyzs = BTreeSet::from_iter(
            old_bes_map
                .keys()
                .into_iter()
                .chain(new_bes_map.keys().into_iter()),
        );
        let map = BTreeMap::from_iter(xyzs.into_iter().map(|xyz| {
            let old = old_bes_map.get(xyz);
            let new = new_bes_map.get(xyz);
            let diff = match (old, new) {
                (None, None) => panic!("block not exists in both old and new block entities"),
                (None, Some((_, v))) => BlockEntityDiff::Create(BlobDiff::from_compare(
                    &Vec::with_capacity(0),
                    &fastnbt::to_bytes(v).unwrap(),
                )),
                (Some((_, v)), None) => BlockEntityDiff::Delete(BlobDiff::from_compare(
                    &fastnbt::to_bytes(v).unwrap(),
                    &Vec::with_capacity(0),
                )),
                (Some((old_id, old_v)), Some((new_id, new_v))) => {
                    if old_id == new_id {
                        BlockEntityDiff::UpdateSameID(MyersDiff::from_compare(
                            &fastnbt::to_bytes(old_v).unwrap(),
                            &fastnbt::to_bytes(new_v).unwrap(),
                        ))
                    } else {
                        BlockEntityDiff::UpdateDiffID(BlobDiff::from_compare(
                            &fastnbt::to_bytes(old_v).unwrap(),
                            &fastnbt::to_bytes(new_v).unwrap(),
                        ))
                    }
                }
            };
            (xyz.clone(), diff)
        }));
        Self {
            old_xyz_list,
            new_xyz_list,
            map,
        }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        let xyzs = BTreeSet::from_iter(
            base.map
                .keys()
                .into_iter()
                .chain(squashing.map.keys().into_iter()),
        );
        let map = BTreeMap::from_iter(xyzs.into_iter().filter_map(|xyz| {
            let base_diff = base.map.get(xyz);
            let squashing_diff = squashing.map.get(xyz);
            let squashed = match (base_diff, squashing_diff) {
                (None, None) => panic!("diff in {:?} not exists in both base and squash", xyz),
                (None, Some(squashing_diff)) => Some(squashing_diff.clone()),
                (Some(base_diff), None) => Some(base_diff.clone()),
                (Some(base_diff), Some(squashing_diff)) => {
                    match (base_diff, squashing_diff) {
                        // Create xor Delete
                        (BlockEntityDiff::Create(_), BlockEntityDiff::Delete(_)) => None,
                        (BlockEntityDiff::Delete(base), BlockEntityDiff::Create(squashing)) => {
                            Some(BlockEntityDiff::UpdateDiffID(BlobDiff::from_squash(
                                base, squashing,
                            )))
                        }

                        // Create then Update
                        (BlockEntityDiff::Create(blob), BlockEntityDiff::UpdateSameID(myers)) => {
                            Some(BlockEntityDiff::Create(BlobDiff::from_create(
                                &myers.patch(blob.get_new_text()),
                            )))
                        }
                        (BlockEntityDiff::Create(_), BlockEntityDiff::UpdateDiffID(blob)) => Some(
                            BlockEntityDiff::Create(BlobDiff::from_create(blob.get_new_text())),
                        ),

                        // Update then Delete
                        (BlockEntityDiff::UpdateSameID(myers), BlockEntityDiff::Delete(blob)) => {
                            Some(BlockEntityDiff::Delete(BlobDiff::from_delete(
                                &myers.revert(blob.get_old_text()),
                            )))
                        }
                        (BlockEntityDiff::UpdateDiffID(blob), BlockEntityDiff::Delete(_)) => Some(
                            BlockEntityDiff::Delete(BlobDiff::from_delete(blob.get_old_text())),
                        ),

                        // Updates in different type
                        (
                            BlockEntityDiff::UpdateSameID(myers),
                            BlockEntityDiff::UpdateDiffID(blob),
                        ) => Some(BlockEntityDiff::UpdateDiffID(BlobDiff::from_compare(
                            &myers.revert(blob.get_old_text()),
                            blob.get_new_text(),
                        ))),
                        (
                            BlockEntityDiff::UpdateDiffID(blob),
                            BlockEntityDiff::UpdateSameID(myers),
                        ) => Some(BlockEntityDiff::UpdateDiffID(BlobDiff::from_compare(
                            blob.get_old_text(),
                            &myers.patch(blob.get_new_text()),
                        ))),

                        // Updates in same type
                        (
                            BlockEntityDiff::UpdateSameID(base),
                            BlockEntityDiff::UpdateSameID(squashing),
                        ) => Some(BlockEntityDiff::UpdateSameID(MyersDiff::from_squash(
                            base, squashing,
                        ))),
                        (
                            BlockEntityDiff::UpdateDiffID(base),
                            BlockEntityDiff::UpdateDiffID(squashing),
                        ) => Some(BlockEntityDiff::UpdateDiffID(BlobDiff::from_squash(
                            base, squashing,
                        ))),

                        // panics
                        _ => {
                            panic!("mismatched base diff and squashing diff")
                        }
                    }
                }
            };
            squashed.map(|diff| (xyz.clone(), diff))
        }));
        Self {
            old_xyz_list: base.old_xyz_list.clone(),
            new_xyz_list: squashing.new_xyz_list.clone(),
            map,
        }
    }

    fn patch(&self, old: &Value) -> Value {
        let mut bes_map = build_bes_map(old);
        for (xyz, diff) in self.map.iter() {
            let old_be = bes_map.get(xyz);
            let new_be = match (old_be, diff) {
                (None, BlockEntityDiff::Create(diff)) => Some(de(&diff.patch0())),
                (Some(_), BlockEntityDiff::Delete(_)) => None,
                (Some(old), BlockEntityDiff::UpdateSameID(diff)) => {
                    Some(de(&diff.patch(&ser(old))))
                }
                (Some(_), BlockEntityDiff::UpdateDiffID(diff)) => Some(de(&diff.patch0())),
                (old_be, diff) => panic!("unmatching {:?} and {:?}", old_be, diff),
            };
            match new_be {
                Some(be) => bes_map.insert(*xyz, be),
                None => bes_map.remove(xyz),
            };
        }
        build_bes_value(bes_map, &self.new_xyz_list)
    }

    fn revert(&self, new: &Value) -> Value {
        let mut bes_map = build_bes_map(new);
        for (xyz, diff) in self.map.iter() {
            let new_be = bes_map.get(xyz);
            let old_be = match (diff, new_be) {
                (BlockEntityDiff::Create(_), Some(_)) => None,
                (BlockEntityDiff::Delete(diff), None) => Some(de(&diff.revert0())),
                (BlockEntityDiff::UpdateSameID(diff), Some(new)) => {
                    Some(de(&diff.revert(&ser(new))))
                }
                (BlockEntityDiff::UpdateDiffID(diff), Some(_)) => Some(de(&diff.revert0())),
                (diff, new_be) => panic!("unmatching {:?} and {:?}", diff, new_be),
            };
            match old_be {
                Some(be) => bes_map.insert(*xyz, be),
                None => bes_map.remove(xyz),
            };
        }
        build_bes_value(bes_map, &self.old_xyz_list)
    }
}
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use fastnbt::Value;

    use crate::{diff::Diff, mca::ChunkWithTimestamp, util::test::get_test_chunk_by_xz};

    use super::BlockEntitiesDiff;

    fn get_block_entities_from_chunk(chunk: ChunkWithTimestamp) -> Value {
        let nbt = chunk.nbt;
        match fastnbt::from_bytes(&nbt).unwrap() {
            Value::Compound(mut map) => map.remove("block_entities").unwrap(),
            _ => panic!("root is not Value::Compound"),
        }
    }
    #[test]
    fn test_diff_patch_revert() {
        let old_chunk = get_test_chunk_by_xz(
            &PathBuf::from("./resources/test-payload/region/mca/hairlessvillager-0/20250515.mca"),
            25,
            29,
        )
        .unwrap();
        let old = get_block_entities_from_chunk(old_chunk);
        let new_chunk = get_test_chunk_by_xz(
            &PathBuf::from("./resources/test-payload/region/mca/hairlessvillager-0/20250516.mca"),
            25,
            29,
        )
        .unwrap();
        let new = get_block_entities_from_chunk(new_chunk);
        let diff = BlockEntitiesDiff::from_compare(&old, &new);
        let patched_old = diff.patch(&old);
        let reverted_new = diff.revert(&new);
        assert_eq!(patched_old, new);
        assert_eq!(reverted_new, old);
    }

    #[test]
    fn test_diff_squash() {
        let mut bes_list = [
            "./resources/test-payload/region/mca/hairlessvillager-0/20250514.mca",
            "./resources/test-payload/region/mca/hairlessvillager-0/20250515.mca",
            "./resources/test-payload/region/mca/hairlessvillager-0/20250516.mca",
        ]
        .map(|path| {
            let chunk = get_test_chunk_by_xz(&PathBuf::from(path), 25, 29).unwrap();
            let bes = get_block_entities_from_chunk(chunk);
            Some(bes)
        });
        let v0 = bes_list[0].take().unwrap();
        let v1 = bes_list[1].take().unwrap();
        let v2 = bes_list[2].take().unwrap();
        let diff_v01 = BlockEntitiesDiff::from_compare(&v0, &v1);
        let diff_v12 = BlockEntitiesDiff::from_compare(&v1, &v2);
        let squashed_diff = BlockEntitiesDiff::from_squash(&diff_v01, &diff_v12);
        let patched_v0 = squashed_diff.patch(&v0);
        let reverted_v2 = squashed_diff.revert(&v2);
        assert_eq!(patched_v0, v2);
        assert_eq!(reverted_v2, v0);
    }
}
