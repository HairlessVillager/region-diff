use hex::encode as hex;
use url::Url;

mod mem;
mod rocksdb;

use crate::err::Error;
pub use mem::Memory;
pub use rocksdb::RocksDB;

// TODO: zero-copy implemention
pub trait StorageBackend {
    fn put_batch<I, K, V>(&mut self, iter: I) -> Result<(), Error>
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>;

    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>;

    fn exists<K>(&self, key: K) -> bool
    where
        K: AsRef<[u8]>;

    fn get<K>(&self, key: K) -> Result<Vec<u8>, Error>
    where
        K: AsRef<[u8]>;

    fn delete<K>(&mut self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>;
}

pub enum WrappedStorageBackend {
    Memory(Memory),
    RocksDB(RocksDB),
}

pub fn create_storage_backend(url: &str) -> WrappedStorageBackend {
    let parsed = Url::parse(url).unwrap();
    match parsed.scheme() {
        "memory" => WrappedStorageBackend::Memory(Memory::new()),
        "rocksdb" => WrappedStorageBackend::RocksDB(RocksDB::new(parsed.path()).unwrap()),

        #[cfg(test)]
        "tempdir" => {
            let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
            let db_path = temp_dir.path();
            WrappedStorageBackend::RocksDB(RocksDB::new_temp(db_path).unwrap())
        }
        _ => panic!("unsupported storage backend scheme"),
    }
}

impl StorageBackend for WrappedStorageBackend {
    fn put_batch<I, K, V>(&mut self, iter: I) -> Result<(), Error>
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        log::debug!("put batch to storage backend");
        match self {
            Self::Memory(x) => x.put_batch(iter),
            Self::RocksDB(x) => x.put_batch(iter),
        }
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        log::debug!("put {} to storage backend", &hex(&key)[..8]);
        match self {
            Self::Memory(x) => x.put(key, value),
            Self::RocksDB(x) => x.put(key, value),
        }
    }

    fn exists<K>(&self, key: K) -> bool
    where
        K: AsRef<[u8]>,
    {
        log::debug!("check {} is exists from storage backend", &hex(&key)[..8]);
        match self {
            Self::Memory(x) => x.exists(key),
            Self::RocksDB(x) => x.exists(key),
        }
    }

    fn get<K>(&self, key: K) -> Result<Vec<u8>, Error>
    where
        K: AsRef<[u8]>,
    {
        log::debug!("get {} from storage backend", &hex(&key)[..8]);
        match self {
            Self::Memory(x) => x.get(key),
            Self::RocksDB(x) => x.get(key),
        }
    }

    fn delete<K>(&mut self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
    {
        log::debug!("delete {} from storage backend", &hex(&key)[..8]);
        match self {
            Self::Memory(x) => x.delete(key),
            Self::RocksDB(x) => x.delete(key),
        }
    }
}
