use rocksdb::{DB, Options, WriteBatch};
use std::path::Path;

use crate::err::Error;

use super::StorageBackend;

pub struct RocksDB {
    db: DB,
}

impl RocksDB {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db =
            DB::open(&opts, path).map_err(|e| Error::from_msg_err("failed to open RocksDB", &e))?;
        Ok(Self { db })
    }
}

impl StorageBackend for RocksDB {
    fn put_batch<I, K, V>(&mut self, iter: I) -> Result<(), Error>
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let mut batch = WriteBatch::default();
        for (key, value) in iter {
            batch.put(key.as_ref(), value.as_ref());
        }
        self.db
            .write(batch)
            .map_err(|e| Error::from_msg_err("failed to write batch to RocksDB", &e))?;
        Ok(())
    }

    fn get<K>(&self, key: K) -> Result<Vec<u8>, Error>
    where
        K: AsRef<[u8]>,
    {
        let res = self
            .db
            .get(key.as_ref())
            .map_err(|e| Error::from_msg_err("failed to get in RocksDB", &e))?;
        match res {
            Some(value) => Ok(value.to_vec()),
            None => Err(Error::from(format!(
                "key {:?} not exists in RocksDB",
                key.as_ref()
            ))),
        }
    }

    fn delete<K>(&self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
    {
        self.db
            .delete(key.as_ref())
            .map_err(|e| Error::from_msg_err("failed to delete in RocksDB", &e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_rocksdb_works() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path();

        if Path::new(db_path).exists() {
            fs::remove_dir_all(db_path).expect("Failed to remove existing database directory");
        }

        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, db_path).expect("Failed to open database");

        let key = b"key1";
        let value = b"value1";
        db.put(key, value).expect("Failed to put value");

        let retrieved_value = db.get(key).expect("Failed to get value");
        assert_eq!(retrieved_value.unwrap(), value);

        let new_value = b"new_value1";
        db.put(key, new_value).expect("Failed to update value");

        let updated_value = db.get(key).expect("Failed to get updated value");
        assert_eq!(updated_value.unwrap(), new_value);

        db.delete(key).expect("Failed to delete value");

        let deleted_value = db.get(key).expect("Failed to check deleted value");
        assert!(deleted_value.is_none());

        let mut batch = WriteBatch::default();
        batch.put(b"key2", b"value2");
        batch.put(b"key3", b"value3");
        db.write(batch).expect("Failed to write batch");

        let value2 = db.get(b"key2").expect("Failed to get value2");
        assert_eq!(value2.unwrap(), b"value2");

        let value3 = db.get(b"key3").expect("Failed to get value3");
        assert_eq!(value3.unwrap(), b"value3");

        temp_dir.close().expect("Failed to clean up temp directory");
    }

    #[test]
    fn test_rocksdb() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path();

        let mut storage = RocksDB::new(db_path).unwrap();

        storage
            .put_batch(vec![(b"key1", b"value1")].into_iter())
            .unwrap();
        let value1 = storage.get(b"key1").unwrap();
        assert_eq!(value1, b"value1");

        storage
            .put_batch(vec![(b"key2", b"value2"), (b"key3", b"value3")].into_iter())
            .unwrap();
        let value2 = storage.get(b"key2").unwrap();
        assert_eq!(value2, b"value2");

        let value3 = storage.get(b"key3").unwrap();
        assert_eq!(value3, b"value3");

        match storage.get(b"nonexistent_key") {
            Ok(_) => panic!("Expected KeyNotFound error"),
            Err(_) => {}
        }

        storage.delete(b"key1").unwrap();
        match storage.get(b"key1") {
            Ok(_) => panic!("Expected KeyNotFound error after deletion"),
            Err(_) => {}
        }

        temp_dir.close().expect("Failed to clean up temp directory");
    }
}
