use std::collections::{BTreeMap, BTreeSet};

use fastnbt::Value;

use crate::{
    diff::{
        Diff,
        base::{BlobDiff, MyersDiff},
    },
    object::Serde,
};
type XYZ = (i32, i32, i32);

#[derive(Debug, Clone)]
enum BlockEntityDiff {
    Create(BlobDiff),
    Delete(BlobDiff),
    UpdateSameID(MyersDiff),
    UpdateDiffID(BlobDiff),
}

#[derive(Debug, Clone)]
pub struct BlockEntitiesDiff {
    map: BTreeMap<XYZ, BlockEntityDiff>,
}
fn build_bes_map(bes: &Value) -> BTreeMap<(i32, i32, i32), (String, &Value)> {
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
                let id = match kv.get("id").unwrap() {
                    Value::String(s) => s.clone(),
                    _ => panic!("be.id should be Value::String"),
                };
                ((x, y, z), (id, be))
            }
            _ => panic!("be should be Value::Compound"),
        })),
        _ => panic!("bes should be Value::List"),
    }
}
impl BlockEntitiesDiff {
    pub fn from_compare(old: &Value, new: &Value) -> Self {
        let old_bes_map = build_bes_map(old);
        let new_bes_map = build_bes_map(new);
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
        Self { map }
    }
}
impl Diff<Value> for BlockEntitiesDiff {
    fn from_compare(old: &Value, new: &Value) -> Self {
        let old_bes_map = build_bes_map(old);
        let new_bes_map = build_bes_map(new);
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
        Self { map }
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized,
    {
        todo!()
    }

    fn patch(&self, old: &Value) -> Value {
        todo!()
    }

    fn revert(&self, new: &Value) -> Value {
        todo!()
    }
}
impl Serde for BlockEntitiesDiff {
    fn serialize(&self) -> Result<Vec<u8>, crate::object::SerdeError> {
        todo!()
    }

    fn deserialize(bytes: &Vec<u8>) -> Result<Self, crate::object::SerdeError>
    where
        Self: Sized,
    {
        todo!()
    }
}
#[cfg(test)]
mod tests {
    use fastnbt::Value;

    use crate::{mca::ChunkWithTimestamp, util::test::get_test_chunk_by_xz};

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
        let old_chunk = get_test_chunk_by_xz("./resources/mca/r.1.2.20250515.mca", 25, 29);
        let old_bes = get_block_entities_from_chunk(old_chunk);
        let new_chunk = get_test_chunk_by_xz("./resources/mca/r.1.2.20250516.mca", 25, 29);
        let new_bes = get_block_entities_from_chunk(new_chunk);
        BlockEntitiesDiff::from_compare(&old_bes, &new_bes);
    }
}
