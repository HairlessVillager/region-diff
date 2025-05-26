use crate::err::Error;

mod rocksdb;

pub use rocksdb::RocksDB;

// TODO: zero-copy implemention
pub trait StorageBackend {
    fn put_batch<I, K, V>(&mut self, iter: I) -> Result<(), Error>
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>;

    fn get<K>(&self, key: K) -> Result<Vec<u8>, Error>
    where
        K: AsRef<[u8]>;

    fn delete<K>(&self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>;
}
