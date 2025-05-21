pub mod mca;
pub mod object;

#[cfg(test)]
mod tests {
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
}
