use super::{Object, ObjectHash};
use crate::diff::{BlobDiff, MyersDiff, RegionDiff};

impl Object for BlobDiff {
    fn hash(&self) -> ObjectHash {
        todo!()
    }
}
impl Object for MyersDiff {
    fn hash(&self) -> ObjectHash {
        todo!()
    }
}
impl Object for RegionDiff {
    fn hash(&self) -> ObjectHash {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
