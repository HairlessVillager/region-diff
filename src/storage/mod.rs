use crate::err::Error;

mod rocksdb;

pub trait StorageBackend<'a> {
    fn put<T>(&mut self, key: T, value: T) -> Result<(), Error>
    where
        T: AsRef<[u8]>;

    fn get<T>(&self, key: T) -> Result<Vec<u8>, Error>
    where
        T: AsRef<[u8]>;

    fn delete<T>(&self, key: T) -> Result<(), Error>
    where
        T: AsRef<[u8]>;
}
