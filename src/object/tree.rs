use super::ObjectHash;
use std::collections::BTreeMap;
type RelativeFilePath = String;

pub struct Tree {
    path2diff: BTreeMap<RelativeFilePath, ObjectHash>,
}
