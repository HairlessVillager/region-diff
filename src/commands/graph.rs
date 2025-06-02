use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use bincode::{Decode, Encode};

use crate::{
    object::{INDEX_HASH, Object, ObjectHash, commit::Commit, index::Index},
    storage::{StorageBackend, WrappedStorageBackend},
};

type Cost = u32;
#[derive(Debug, Clone, Encode, Decode)]
pub struct EdgeCost {
    pub patch: Cost,
    pub revert: Cost,
}

pub struct CommitGraph<T> {
    commits: HashMap<Rc<T>, ()>,

    // should be a DAG
    adj_list: HashMap<Rc<T>, HashMap<Rc<T>, EdgeCost>>,
}

#[derive(Debug, PartialEq)]
pub enum ApplyEdge<T> {
    Patch(Rc<T>),
    Revert(Rc<T>),
}

impl<T: Eq + Hash + Clone> CommitGraph<T> {
    pub fn add_commit(&mut self, commit: T) -> Rc<T> {
        if let Some(existing) = self.commits.keys().find(|rc| ***rc == commit).cloned() {
            return existing;
        }
        let rc = Rc::new(commit);
        self.commits.insert(rc.clone(), ());
        rc
    }
    pub fn get_or_add_commit(&mut self, commit: &T) -> Rc<T> {
        self.commits
            .keys()
            .find(|rc| ***rc == *commit)
            .cloned()
            .unwrap_or_else(|| self.add_commit(commit.clone()))
    }
    pub fn add_edge(&mut self, old: &T, new: &T, cost: EdgeCost) {
        let old_rc = self.get_or_add_commit(old);
        let new_rc = self.get_or_add_commit(new);
        self.adj_list
            .entry(new_rc)
            .or_default()
            .insert(old_rc, cost);
    }
    fn dijkstra(&self, s: Rc<T>, w: impl Fn(&EdgeCost) -> Cost) -> HashMap<Rc<T>, (Cost, Rc<T>)> {
        // todo: use heap to be more efficiently
        let mut done_map = HashMap::new();
        let mut todo_map = HashMap::new();
        todo_map.insert(s.clone(), (0, s.clone()));

        // get commit with min cost
        while let Some((commit, (cost, _prev))) = todo_map.iter().min_by_key(|(_, (cost, _))| *cost)
        {
            let commit = commit.clone();
            let cost = *cost;

            // move it from todo_map to done_map
            let e = todo_map.remove_entry(&commit).unwrap();
            done_map.insert(e.0, e.1);

            // update todo_map
            let edges = if let Some(edges) = self.adj_list.get(&commit) {
                edges
            } else {
                continue;
            };
            for (parent, ec) in edges {
                if done_map.contains_key(parent) {
                    log::warn!("DAG should not contains a circuit");
                    continue;
                }
                let delta_cost = w(ec);
                todo_map
                    .entry(parent.clone())
                    .and_modify(|e| {
                        if cost + delta_cost < e.0 {
                            e.0 = cost + delta_cost;
                            e.1 = commit.clone();
                        }
                    })
                    .or_insert((cost + delta_cost, commit.clone()));
            }
        }

        done_map.remove(&s);
        done_map
    }
    pub fn shortest_path(&self, s: Rc<T>, t: Rc<T>) -> Vec<ApplyEdge<T>> {
        // build ancestors for two directions
        let ancestors_s: HashMap<Rc<T>, (u32, Rc<T>)> = self.dijkstra(s, |ec| ec.revert);
        let ancestors_t: HashMap<Rc<T>, (u32, Rc<T>)> =
            self.dijkstra(t.clone() /*  */, |ec| ec.patch);

        // find min-cost ca
        let common_ancestors = ancestors_s
            .keys()
            .filter(|k| ancestors_t.contains_key(k.clone()));
        let (ca, _, _) = common_ancestors
            .map(|ca| (ca.clone(), ancestors_s[ca].clone(), ancestors_t[ca].clone()))
            .min_by_key(|(_, (revert_cost, _), (patch_cost, _))| *revert_cost + *patch_cost)
            .unwrap();

        // build path
        let mut path = Vec::new();
        let mut curr = ca.clone();
        while let Some((_, prev)) = ancestors_s.get(&curr) {
            path.push(ApplyEdge::Revert(prev.clone()));
            curr = prev.clone();
        }
        path.pop();
        path.reverse();
        path.push(ApplyEdge::Revert(ca.clone()));
        let mut curr = ca.clone();
        while let Some((_, prev)) = ancestors_t.get(&curr) {
            path.push(ApplyEdge::Patch(prev.clone()));
            curr = prev.clone();
        }
        path
    }
}

type CommitHash = ObjectHash;
pub fn graph(backend: &WrappedStorageBackend) -> CommitGraph<CommitHash> {
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);
    let mut graph = CommitGraph {
        commits: HashMap::new(),
        adj_list: HashMap::new(),
    };

    let mut refs_stack: Vec<Rc<CommitHash>> = Vec::new();
    for commit in index.get_all_refs() {
        let converted = graph.get_or_add_commit(commit);
        refs_stack.push(converted);
    }
    let mut visited: HashSet<Rc<CommitHash>> = HashSet::new();
    while let Some(top) = refs_stack.pop() {
        if visited.contains(&top) {
            continue;
        }
        let commit = backend.get(&*top).unwrap();
        let commit = Commit::deserialize(&commit);
        let edges = commit.get_edges();
        let old: Rc<CommitHash> = graph.get_or_add_commit(&top);
        for (edge_commit, (_, edge_cost)) in edges {
            let new: Rc<CommitHash> = graph.get_or_add_commit(edge_commit);
            if !visited.contains(&new) {
                refs_stack.push(new.clone());
            }
            graph.add_edge(&old, &new, edge_cost.clone());
        }
        visited.insert(top);
    }
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestHash = String;

    mod test_dijkstra {
        use super::*;
        use std::rc::Rc;

        fn build_test_graph() -> CommitGraph<TestHash> {
            let mut graph = CommitGraph::<TestHash> {
                commits: HashMap::new(),
                adj_list: HashMap::new(),
            };

            let s = graph.add_commit("S".into());
            let t = graph.add_commit("T".into());
            let v1s = graph.add_commit("V_1S".into());
            let v1t = graph.add_commit("V_1T".into());
            let v2 = graph.add_commit("V_2".into());
            let v3 = graph.add_commit("V_3".into());
            let v4 = graph.add_commit("V_4".into());

            let unit_cost = EdgeCost {
                patch: 1,
                revert: 1,
            };

            graph.add_edge(&v4, &v3, unit_cost.clone());
            graph.add_edge(&v3, &v1s, unit_cost.clone());
            graph.add_edge(&v3, &v2, unit_cost.clone());
            graph.add_edge(&v1s, &s, unit_cost.clone());
            graph.add_edge(&v2, &v1s, unit_cost.clone());
            graph.add_edge(&v2, &v1t, unit_cost.clone());
            graph.add_edge(&v2, &t, unit_cost.clone());
            graph.add_edge(&v1t, &t, unit_cost.clone());

            graph
        }

        #[test]
        fn test_from_s() {
            let graph = build_test_graph();
            let start = Rc::new("S".into());
            let paths = graph.dijkstra(start, |ec| ec.patch);
            let cases = [
                ("V_1S", 1, "S"),
                ("V_2", 2, "V_1S"),
                ("V_3", 2, "V_1S"),
                ("V_4", 3, "V_3"),
            ];
            for case in cases {
                assert_eq!(
                    paths.get(&Rc::new(case.0.into())),
                    Some(&(case.1, Rc::new(case.2.into())))
                );
            }
            assert_eq!(paths.len(), 4);
        }

        #[test]
        fn test_from_t() {
            let graph = build_test_graph();
            let start = Rc::new("T".into());
            let paths = graph.dijkstra(start, |ec| ec.revert);
            let cases = [
                ("V_1T", 1, "T"),
                ("V_2", 1, "T"),
                ("V_3", 2, "V_2"),
                ("V_4", 3, "V_3"),
            ];
            for case in cases {
                assert_eq!(
                    paths.get(&Rc::new(case.0.into())),
                    Some(&(case.1, Rc::new(case.2.into())))
                );
            }
            assert_eq!(paths.len(), 4);
        }

        #[test]
        fn test_isolated_node() {
            let mut graph = CommitGraph::<TestHash> {
                commits: HashMap::new(),
                adj_list: HashMap::new(),
            };

            graph.add_commit("isolated_node".into());

            let paths = graph.dijkstra(Rc::new("ISO".into()), |ec| ec.patch);
            assert!(paths.is_empty());
        }
    }
    mod test_shortest_path {
        use super::*;

        fn build_test_graph(complex: bool) -> CommitGraph<TestHash> {
            let mut graph = CommitGraph::<TestHash> {
                commits: HashMap::new(),
                adj_list: HashMap::new(),
            };

            let s = graph.add_commit("S".into());
            let t = graph.add_commit("T".into());
            let v1s = graph.add_commit("V_1S".into());
            let v2s = graph.add_commit("V_2S".into());
            let v3s = graph.add_commit("V_3S".into());
            let v1t = graph.add_commit("V_1T".into());
            let v2t = graph.add_commit("V_2T".into());
            let v3t = graph.add_commit("V_3T".into());
            let v4 = graph.add_commit("V_4".into());
            let v5 = graph.add_commit("V_5".into());
            let v6 = graph.add_commit("V_6".into());
            let v7 = graph.add_commit("V_7".into());

            let unit_cost = EdgeCost {
                patch: 1,
                revert: 1,
            };

            graph.add_edge(&v7, &v6, unit_cost.clone());
            graph.add_edge(&v6, &v5, unit_cost.clone());
            graph.add_edge(&v5, &v4, unit_cost.clone());
            graph.add_edge(&v4, &v3s, unit_cost.clone());
            graph.add_edge(&v4, &v3t, unit_cost.clone());
            graph.add_edge(&v3s, &v2s, unit_cost.clone());
            graph.add_edge(&v3t, &v2t, unit_cost.clone());
            graph.add_edge(&v2s, &v1s, unit_cost.clone());
            graph.add_edge(&v2t, &v1t, unit_cost.clone());
            graph.add_edge(&v1s, &s, unit_cost.clone());
            graph.add_edge(&v1t, &t, unit_cost.clone());

            if complex {
                graph.add_edge(&v6, &v1s, unit_cost.clone());
                graph.add_edge(&v6, &v1t, unit_cost.clone());
            }

            graph
        }
        #[test]
        fn test_simple_graph() {
            let graph = build_test_graph(false);
            let s = Rc::new("S".into());
            let t = Rc::new("T".into());
            let path = graph.shortest_path(s, t);
            assert_eq!(
                path,
                vec![
                    ApplyEdge::Revert(Rc::new("V_1S".into())),
                    ApplyEdge::Revert(Rc::new("V_2S".into())),
                    ApplyEdge::Revert(Rc::new("V_3S".into())),
                    ApplyEdge::Revert(Rc::new("V_4".into())),
                    ApplyEdge::Patch(Rc::new("V_3T".into())),
                    ApplyEdge::Patch(Rc::new("V_2T".into())),
                    ApplyEdge::Patch(Rc::new("V_1T".into())),
                    ApplyEdge::Patch(Rc::new("T".into())),
                ]
            );
        }
        #[test]
        fn test_complex_graph() {
            let graph = build_test_graph(true);
            let s = Rc::new("S".into());
            let t = Rc::new("T".into());
            let path = graph.shortest_path(s, t);
            assert_eq!(
                path,
                vec![
                    ApplyEdge::Revert(Rc::new("V_1S".into())),
                    ApplyEdge::Revert(Rc::new("V_6".into())),
                    ApplyEdge::Patch(Rc::new("V_1T".into())),
                    ApplyEdge::Patch(Rc::new("T".into())),
                ]
            );
        }
    }
}
