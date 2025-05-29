use crate::err::Error;

mod mem;
mod rocksdb;

pub use mem::Memory;
pub use rocksdb::RocksDB;
use url::Url;

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

    fn delete<K>(&mut self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>;
}

pub enum WrappedStorageBackend {
    Memory(Memory),
    RocksDB(RocksDB),
}

impl WrappedStorageBackend {
    pub fn new(url: &str) -> WrappedStorageBackend {
        let parsed = Url::parse(url).unwrap();
        match parsed.scheme() {
            "memory" => Self::Memory(Memory::new()),
            "rocksdb" => Self::RocksDB(RocksDB::new(parsed.path()).unwrap()),
            _ => panic!("unsupported storage backend scheme"),
        }
    }
}

impl StorageBackend for WrappedStorageBackend {
    fn put_batch<I, K, V>(&mut self, iter: I) -> Result<(), Error>
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        match self {
            Self::Memory(x) => x.put_batch(iter),
            Self::RocksDB(x) => x.put_batch(iter),
        }
    }

    fn get<K>(&self, key: K) -> Result<Vec<u8>, Error>
    where
        K: AsRef<[u8]>,
    {
        match self {
            Self::Memory(x) => x.get(key),
            Self::RocksDB(x) => x.get(key),
        }
    }

    fn delete<K>(&mut self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
    {
        match self {
            Self::Memory(x) => x.delete(key),
            Self::RocksDB(x) => x.delete(key),
        }
    }
}
