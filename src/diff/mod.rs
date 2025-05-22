pub mod blob;
pub mod myers;
pub mod region;
pub use blob::BlobDiff;
pub use myers::MyersDiff;
pub use region::RegionDiff;

use crate::object::Serde;

pub trait Diff: Serde {
    fn from_compare(old: &[u8], new: &[u8]) -> Self
    where
        Self: Sized;
    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized;
    fn patch(&self, old: &[u8]) -> Vec<u8>;
    fn revert(&self, new: &[u8]) -> Vec<u8>;
}
