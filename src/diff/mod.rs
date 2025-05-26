pub mod base;
pub mod file;
pub mod nbt;

use bincode::{Decode, Encode};

pub trait Diff<T>: Encode + Decode<Self> + Clone {
    fn from_compare(old: &T, new: &T) -> Self
    where
        Self: Sized;
    fn from_squash(base: &Self, squashing: &Self) -> Self
    where
        Self: Sized;
    fn patch(&self, old: &T) -> T;
    fn revert(&self, new: &T) -> T;
}
