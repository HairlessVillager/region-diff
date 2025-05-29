use super::StorageBackend;
use crate::err::Error;
use std::collections::BTreeMap;

pub struct Memory {
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }
}

impl StorageBackend for Memory {
    fn put_batch<I, K, V>(&mut self, iter: I) -> Result<(), Error>
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        for (key, value) in iter {
            let key_bytes = key.as_ref().to_vec();
            let value_bytes = value.as_ref().to_vec();
            self.map.insert(key_bytes, value_bytes);
        }
        Ok(())
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let key_bytes = key.as_ref().to_vec();
        let value_bytes = value.as_ref().to_vec();
        self.map.insert(key_bytes, value_bytes);
        Ok(())
    }

    fn get<K>(&self, key: K) -> Result<Vec<u8>, Error>
    where
        K: AsRef<[u8]>,
    {
        let key_bytes = key.as_ref();
        self.map
            .get(key_bytes)
            .cloned()
            .ok_or_else(|| Error::from(format!("key {:?} not exists in Memory storage", key_bytes)))
    }

    fn delete<K>(&mut self, key: K) -> Result<(), Error>
    where
        K: AsRef<[u8]>,
    {
        let key_bytes = key.as_ref();
        self.map
            .remove(key_bytes)
            .map(|_| ())
            .ok_or_else(|| Error::from(format!("key {:?} not exists in Memory storage", key_bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_storage() {
        let mut storage = Memory::new();

        storage.put(b"key1", b"value1").unwrap();
        assert_eq!(storage.get(b"key1").unwrap(), b"value1");

        storage
            .put_batch(vec![(b"key2", b"value2"), (b"key3", b"value3")].into_iter())
            .unwrap();
        assert_eq!(storage.get(b"key2").unwrap(), b"value2");
        assert_eq!(storage.get(b"key3").unwrap(), b"value3");

        storage
            .put_batch(vec![(b"key1", b"new_value1")].into_iter())
            .unwrap();
        assert_eq!(storage.get(b"key1").unwrap(), b"new_value1");

        storage.delete(b"key1").unwrap();
        assert!(storage.get(b"key1").is_err());

        assert!(storage.delete(b"nonexistent").is_err());

        assert!(storage.get(b"invalid").is_err());
    }
}
