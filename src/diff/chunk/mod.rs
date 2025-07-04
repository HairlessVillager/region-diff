mod entities;
mod region;

pub use entities::EntitiesChunkDiff;
pub use region::RegionChunkDiff;

#[cfg(test)]
mod tests {
    use fastnbt::nbt;

    #[test]
    fn test_fastnbt_works() {
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
}
