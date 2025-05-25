mod rocksdb;

pub struct StorageBackendError {
    msg: String,
}
impl From<&str> for StorageBackendError {
    fn from(value: &str) -> Self {
        Self {
            msg: value.to_string(),
        }
    }
}
pub trait StorageBackend<'a> {
    fn put<T>(&mut self, key: T, value: T) -> Result<(), StorageBackendError>
    where
        T: AsRef<[u8]>;

    fn get<T>(&self, key: T) -> Result<Vec<u8>, StorageBackendError>
    where
        T: AsRef<[u8]>;

    fn delete<T>(&self, key: T) -> Result<(), StorageBackendError>
    where
        T: AsRef<[u8]>;
}
