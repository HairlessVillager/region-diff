use bincode::{Decode, Encode};
use similar::{Algorithm, DiffOp, capture_diff_slices};
use std::io::{Cursor, Read, Seek};

use crate::diff::Diff;

#[derive(Debug, Encode, Decode, PartialEq, Clone)]
pub struct MyersDiff {
    old_text: Vec<u8>,
    new_text: Vec<u8>,
    replaces: Vec<Replace>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
struct Replace {
    old_idx: usize,
    old_len: usize,
    new_idx: usize,
    new_len: usize,
}

#[derive(Debug)]
struct BaseReplaceEndpoint {
    v0_idx: usize,
    v1_idx: usize,
}
#[derive(Debug)]
struct SquashingReplaceEndpoint {
    v1_idx: usize,
    v2_idx: usize,
}

#[derive(Debug)]
enum NamedReplaceEndpoint {
    BO(BaseReplaceEndpoint),      // Base Diff Openning
    BC(BaseReplaceEndpoint),      // Base Diff Closed
    SO(SquashingReplaceEndpoint), // Squashing Diff Openning
    SC(SquashingReplaceEndpoint), // Squashing Diff Closed
}

#[derive(Debug)]
enum VxPtr {
    Enable(usize),
    Disable(usize),
}

impl Diff<Vec<u8>> for MyersDiff {
    fn from_compare(old: &Vec<u8>, new: &Vec<u8>) -> Self {
        let mut diff = Self {
            old_text: Vec::new(),
            new_text: Vec::new(),
            replaces: Vec::new(),
        };
        let ops = capture_diff_slices(Algorithm::Myers, old, new);
        let mut old_ptr = 0;
        let mut new_ptr = 0;
        let replace_iter = ops.iter().filter_map(|op| match op {
            DiffOp::Equal {
                old_index: _,
                new_index: _,
                len,
            } => {
                old_ptr += len;
                new_ptr += len;
                None
            }
            DiffOp::Insert {
                old_index: _,
                new_index: _,
                new_len,
            } => {
                let r = Some(Replace {
                    old_idx: old_ptr,
                    old_len: 0,
                    new_idx: new_ptr,
                    new_len: *new_len,
                });
                new_ptr += new_len;
                r
            }
            DiffOp::Delete {
                old_index: _,
                old_len,
                new_index: _,
            } => {
                let r = Some(Replace {
                    old_idx: old_ptr,
                    old_len: *old_len,
                    new_idx: new_ptr,
                    new_len: 0,
                });
                old_ptr += old_len;
                r
            }
            DiffOp::Replace {
                old_index: _,
                old_len,
                new_index: _,
                new_len,
            } => {
                let r = Some(Replace {
                    old_idx: old_ptr,
                    old_len: *old_len,
                    new_idx: new_ptr,
                    new_len: *new_len,
                });
                old_ptr += old_len;
                new_ptr += new_len;
                r
            }
        });
        for replace in replace_iter {
            diff.old_text
                .extend_from_slice(&old[replace.old_idx..replace.old_idx + replace.old_len]);
            diff.new_text
                .extend_from_slice(&new[replace.new_idx..replace.new_idx + replace.new_len]);
            diff.replaces.push(replace);
        }
        diff
    }

    fn from_squash(base: &Self, squashing: &Self) -> Self {
        let endpoints = Self::build_endpoints(&base, &squashing);
        Self::build_diff(&base, &squashing, &endpoints)
    }

    fn patch(&self, old: &Vec<u8>) -> Vec<u8> {
        let capacity = old.len() - self.old_text.len() + self.new_text.len();
        let mut patched = Vec::with_capacity(capacity);

        let mut old_ptr: usize = 0;
        let mut new_text_ptr: usize = 0;
        for replace in &self.replaces {
            patched.extend_from_slice(&old[old_ptr..replace.old_idx]);
            patched.extend_from_slice(&self.new_text[new_text_ptr..new_text_ptr + replace.new_len]);
            old_ptr = replace.old_idx + replace.old_len;
            new_text_ptr += replace.new_len;
        }
        patched.extend_from_slice(&old[old_ptr..]);

        patched
    }

    fn revert(&self, new: &Vec<u8>) -> Vec<u8> {
        let capacity = new.len() - self.new_text.len() + self.old_text.len();
        let mut patched = Vec::with_capacity(capacity);

        let mut new_ptr: usize = 0;
        let mut old_text_ptr: usize = 0;
        for replace in &self.replaces {
            patched.extend_from_slice(&new[new_ptr..replace.new_idx]);
            patched.extend_from_slice(&self.old_text[old_text_ptr..old_text_ptr + replace.old_len]);
            new_ptr = replace.new_idx + replace.new_len;
            old_text_ptr += replace.old_len;
        }
        patched.extend_from_slice(&new[new_ptr..]);

        patched
    }
}

impl MyersDiff {
    fn build_endpoints(base: &Self, squashing: &Self) -> Vec<NamedReplaceEndpoint> {
        let mut endpoints: Vec<NamedReplaceEndpoint> = base
            .replaces
            .iter()
            .map(|r| {
                vec![
                    NamedReplaceEndpoint::BO(BaseReplaceEndpoint {
                        v0_idx: r.old_idx,
                        v1_idx: r.new_idx,
                    }),
                    NamedReplaceEndpoint::BC(BaseReplaceEndpoint {
                        v0_idx: r.old_idx + r.old_len,
                        v1_idx: r.new_idx + r.new_len,
                    }),
                ]
            })
            .chain(squashing.replaces.iter().map(|r| {
                vec![
                    NamedReplaceEndpoint::SO(SquashingReplaceEndpoint {
                        v1_idx: r.old_idx,
                        v2_idx: r.new_idx,
                    }),
                    NamedReplaceEndpoint::SC(SquashingReplaceEndpoint {
                        v1_idx: r.old_idx + r.old_len,
                        v2_idx: r.new_idx + r.new_len,
                    }),
                ]
            }))
            .into_iter()
            .flatten()
            .collect();
        endpoints.sort_by_key(|e| match e {
            NamedReplaceEndpoint::BO(r) => r.v1_idx,
            NamedReplaceEndpoint::BC(r) => r.v1_idx,
            NamedReplaceEndpoint::SO(r) => r.v1_idx,
            NamedReplaceEndpoint::SC(r) => r.v1_idx,
        });
        endpoints
    }
    fn build_diff(base: &Self, squashing: &Self, endpoints: &Vec<NamedReplaceEndpoint>) -> Self {
        let mut diff = Self {
            old_text: Vec::new(),
            new_text: Vec::new(),
            replaces: Vec::new(),
        };

        let mut v0_ptr = VxPtr::Disable(0);
        let mut v1_ptr = VxPtr::Disable(0);
        let mut v2_ptr = VxPtr::Disable(0);
        let mut base_old_text = Cursor::new(&base.old_text);
        let mut base_new_text = Cursor::new(&base.new_text);
        let mut squashing_old_text = Cursor::new(&squashing.old_text);
        let mut squashing_new_text = Cursor::new(&squashing.new_text);
        let mut diff_counter = 0u8;
        let mut last_diff_counter = 0u8;
        let mut old_text_ptr = 0;
        let mut new_text_ptr = 0;
        let mut old_idx = 0;
        let mut new_idx = 0;

        for nre in endpoints {
            // write diff text
            match &nre {
                NamedReplaceEndpoint::BO(re) => {
                    match v0_ptr {
                        VxPtr::Disable(_) => v0_ptr = VxPtr::Enable(re.v0_idx),
                        VxPtr::Enable(_) => {
                            panic!("v0_ptr is not disabled (but ={:?}) when met BO", v0_ptr)
                        }
                    }
                    match v1_ptr {
                        VxPtr::Disable(_) => v1_ptr = VxPtr::Enable(re.v1_idx),
                        VxPtr::Enable(ptr) => {
                            let size = re.v1_idx - ptr;
                            let mut buffer = vec![0; size];
                            squashing_old_text.read_exact(&mut buffer).unwrap();
                            diff.old_text.extend_from_slice(&buffer);
                            v1_ptr = VxPtr::Disable(re.v1_idx);
                        }
                    }
                    diff_counter += 1;
                }
                NamedReplaceEndpoint::BC(re) => {
                    match v0_ptr {
                        VxPtr::Disable(_) => {
                            panic!("v0_ptr is not enabled (but ={:?}) when met BO", v0_ptr)
                        }
                        VxPtr::Enable(ptr) => {
                            let size = re.v0_idx - ptr;
                            let mut buffer = vec![0; size];
                            base_old_text.read_exact(&mut buffer).unwrap();
                            diff.old_text.extend_from_slice(&buffer);
                            v0_ptr = VxPtr::Disable(re.v0_idx);
                        }
                    }
                    match v1_ptr {
                        VxPtr::Disable(ptr) => {
                            let step = re.v1_idx - ptr;
                            base_new_text.seek_relative(step as i64).unwrap();
                            squashing_old_text.seek_relative(step as i64).unwrap();
                            v1_ptr = VxPtr::Enable(re.v1_idx);
                        }
                        VxPtr::Enable(ptr) => {
                            let size = re.v1_idx - ptr;
                            let mut buffer = vec![0; size];
                            base_new_text.read_exact(&mut buffer).unwrap();
                            diff.new_text.extend_from_slice(&buffer);
                            v1_ptr = VxPtr::Disable(re.v1_idx);
                        }
                    }
                    diff_counter -= 1;
                }
                NamedReplaceEndpoint::SO(re) => {
                    match v1_ptr {
                        VxPtr::Disable(_) => v1_ptr = VxPtr::Enable(re.v1_idx),
                        VxPtr::Enable(ptr) => {
                            let size = re.v1_idx - ptr;
                            let mut buffer = vec![0; size];
                            base_new_text.read_exact(&mut buffer).unwrap();
                            diff.new_text.extend_from_slice(&buffer);
                            v1_ptr = VxPtr::Disable(re.v1_idx);
                        }
                    }
                    match v2_ptr {
                        VxPtr::Disable(_) => v2_ptr = VxPtr::Enable(re.v2_idx),
                        VxPtr::Enable(ptr) => {
                            panic!("v2_ptr is not closed (={}) when met MO", ptr)
                        }
                    }
                    diff_counter += 1;
                }
                NamedReplaceEndpoint::SC(re) => {
                    match v1_ptr {
                        VxPtr::Disable(ptr) => {
                            let step = re.v1_idx - ptr;
                            base_new_text.seek_relative(step as i64).unwrap();
                            squashing_old_text.seek_relative(step as i64).unwrap();
                            v1_ptr = VxPtr::Enable(re.v1_idx);
                        }
                        VxPtr::Enable(ptr) => {
                            let size = re.v1_idx - ptr;
                            let mut buffer = vec![0; size];
                            squashing_old_text.read_exact(&mut buffer).unwrap();
                            diff.old_text.extend_from_slice(&buffer);
                            v1_ptr = VxPtr::Disable(re.v1_idx);
                        }
                    }
                    match v2_ptr {
                        VxPtr::Disable(_) => {
                            panic!("v2_ptr is not enabled (but ={:?}) when met MO", v2_ptr)
                        }
                        VxPtr::Enable(ptr) => {
                            let size = re.v2_idx - ptr;
                            let mut buffer = vec![0; size];
                            squashing_new_text.read_exact(&mut buffer).unwrap();
                            diff.new_text.extend_from_slice(&buffer);
                            v2_ptr = VxPtr::Disable(re.v2_idx);
                        }
                    }
                    diff_counter -= 1;
                }
            };

            // append replace entry
            if last_diff_counter > 0 && diff_counter == 0 {
                let old_len = diff.old_text.len() - old_text_ptr;
                let new_len = diff.new_text.len() - new_text_ptr;
                diff.replaces.push(Replace {
                    old_idx,
                    old_len,
                    new_idx,
                    new_len,
                });
                old_text_ptr = diff.old_text.len();
                new_text_ptr = diff.new_text.len();
                old_idx += old_len;
                new_idx += new_len;
            } else if last_diff_counter == 0 && diff_counter > 0 {
                let step = match &nre {
                    NamedReplaceEndpoint::BO(re) => re.v0_idx - old_idx,
                    NamedReplaceEndpoint::SO(re) => re.v2_idx - new_idx,
                    _ => panic!("Starting new diff with BC or MC"),
                };
                old_idx += step;
                new_idx += step;
            }
            last_diff_counter = diff_counter;
        }

        diff
    }
}

#[cfg(test)]
mod tests {
    use similar::{Algorithm, DiffOp, capture_diff_slices};

    use crate::util::test::create_test_bytes;

    use super::*;

    #[test]
    fn test_similar_works() {
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
    fn test_diff_patch_revert() -> () {
        let mut old_iter = create_test_bytes(114514);
        let mut new_iter = create_test_bytes(1919810);
        for _ in 0..100_000 {
            let old = old_iter.next().unwrap();
            let new = new_iter.next().unwrap();
            let diff = MyersDiff::from_compare(&old, &new);
            let patched_old = diff.patch(&old);
            let reverted_new = diff.revert(&new);
            assert_eq!(patched_old, new, "old: {:?}; new: {:?}", old, new);
            assert_eq!(reverted_new, old, "old: {:?}; new: {:?}", old, new);
        }
    }
    #[test]
    fn test_diff_squash() -> () {
        let mut v0_iter = create_test_bytes(114514);
        let mut v1_iter = create_test_bytes(1919810);
        let mut v2_iter = create_test_bytes(19260817);
        for _ in 0..100_000 {
            let v0 = v0_iter.next().unwrap();
            let v1 = v1_iter.next().unwrap();
            let v2 = v2_iter.next().unwrap();
            let diff_v01 = MyersDiff::from_compare(&v0, &v1);
            let diff_v12 = MyersDiff::from_compare(&v1, &v2);
            let squashed_diff = MyersDiff::from_squash(&diff_v01, &diff_v12);
            let patched_v0 = squashed_diff.patch(&v0);
            let reverted_v2 = squashed_diff.revert(&v2);
            assert_eq!(patched_v0, v2, "v0: {:?}; v1{:?}; v2: {:?}", v0, v1, v2);
            assert_eq!(reverted_v2, v0, "v0: {:?}; v1{:?}; v2: {:?}", v0, v1, v2);
        }
    }
}
