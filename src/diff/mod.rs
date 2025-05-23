pub mod base;
pub mod file;
pub mod nbt;

use crate::object::Serde;

pub trait Diff: Serde + Clone {
    fn from_compare(old: &[u8], new: &[u8]) -> Self
    where
        Self: Sized;
    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized;
    fn patch(&self, old: &[u8]) -> Vec<u8>;
    fn revert(&self, new: &[u8]) -> Vec<u8>;
}
