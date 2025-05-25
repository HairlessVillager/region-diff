pub mod diff;
pub mod err;
pub mod mca;
pub mod object;
pub mod storage;
pub mod util;

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use rocksdb::{DB, Options, WriteBatch};

    #[test]
    fn test_fastnbt_works() {
        use fastnbt::nbt;

        let a = nbt!({
            "string": "Hello World",
            "number": 42,
            "nested": {
                "array": [1, 2, 3, 4, 5],
                "compound": {
                    "name": "test",
                    "value": 3.14,
                    "list": ["a", "b", "c"]
                }
            },
            "boolean": 1_i8,
            "long_array": [1_i64, 2_i64, 3_i64]
        });
        let a = fastnbt::to_bytes(&a).unwrap();

        let b = nbt!({
            "string": "Hello World",
            "nested": {
                "compound": {
                    "value": 3.14,
                    "name": "test",
                    "list": ["a", "b", "c"]
                },
                "array": [1, 2, 3, 4, 5]
            },
            "number": 42,
            "boolean": 1_i8,
            "long_array": [1_i64, 2_i64, 3_i64]
        });
        let b = fastnbt::to_bytes(&b).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_similar_works() {
        use similar::{Algorithm, DiffOp, capture_diff_slices};

        let a = vec![1, 2, 3];
        let b = vec![3, 2, 1, 2];
        let ops = capture_diff_slices(Algorithm::Myers, &a, &b);
        for op in &ops {
            match op {
                DiffOp::Equal {
                    old_index: _,
                    new_index: _,
                    len: _,
                } => (),
                DiffOp::Insert {
                    old_index: _,
                    new_index: _,
                    new_len: _,
                } => (),
                DiffOp::Delete {
                    old_index: _,
                    old_len: _,
                    new_index: _,
                } => (),
                DiffOp::Replace {
                    old_index: _,
                    old_len: _,
                    new_index: _,
                    new_len: _,
                } => (),
            }
        }
        assert_eq!(
            &ops,
            &vec![
                DiffOp::Insert {
                    old_index: 0,
                    new_index: 0,
                    new_len: 2
                },
                DiffOp::Equal {
                    old_index: 0,
                    new_index: 2,
                    len: 2
                },
                DiffOp::Delete {
                    old_index: 2,
                    old_len: 1,
                    new_index: 4
                },
            ]
        );
    }

    #[test]
    fn test_rocksdb_works() {
        let db_path = "test_rocksdb";

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

        fs::remove_dir_all(db_path).expect("Failed to remove database directory after test");
    }
}
