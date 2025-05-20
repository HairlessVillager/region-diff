pub mod diff;
pub mod mca;

#[cfg(test)]
mod tests {
    use crate::mca::MCAReader;
    #[test]
    fn test_fastnbt_works() -> Result<(), Box<dyn std::error::Error>> {
        use fastnbt::{Value, nbt};
        let x = nbt!({
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

        let y = fastnbt::to_bytes(&x)?;
        let z: Value = fastnbt::from_bytes(&y)?;
        let w = fastnbt::to_bytes(&z)?;
        assert_eq!(y, w);
        assert_eq!(format!("{:?}", x), format!("{:?}", z));

        Ok(())
    }

    #[test]
    fn test_similar_works() -> Result<(), Box<dyn std::error::Error>> {
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

        Ok(())
    }

    #[test]
    fn benchmark() -> Result<(), Box<dyn std::error::Error>> {
        use similar::{Algorithm, DiffOp, capture_diff_slices};
        let files = vec![
            "./mca-test-data/r.1.2.20250511.mca",
            "./mca-test-data/r.1.2.20250516.mca",
        ];
        let nbts = files
            .iter()
            .map(|file| {
                let mut reader = MCAReader::from_file(file, true).unwrap();
                let chunk = reader.get_chunk(15, 20).unwrap();
                let nbt = match chunk {
                    Some(chunk) => &chunk.nbt,
                    _ => panic!("Chunk should be Some"),
                };
                nbt.clone()
            })
            .collect::<Vec<_>>();

        let ops = capture_diff_slices(Algorithm::Myers, &nbts[0], &nbts[1]);
        let mut ops_count: usize = 0;
        let mut insert_count: usize = 0;
        let mut delete_count: usize = 0;
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
                    new_len,
                } => {
                    ops_count += 1;
                    insert_count += new_len
                }
                DiffOp::Delete {
                    old_index: _,
                    old_len,
                    new_index: _,
                } => {
                    ops_count += 1;
                    delete_count += old_len
                }
                DiffOp::Replace {
                    old_index: _,
                    old_len,
                    new_index: _,
                    new_len,
                } => {
                    ops_count += 1;
                    insert_count += new_len;
                    delete_count += old_len
                }
            }
        }
        println!("ops_count: {}", ops_count);
        println!("insert_count: {}", insert_count);
        println!("delete_count: {}", delete_count);
        Ok(())
    }
}
