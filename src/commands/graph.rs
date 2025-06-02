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
    pub fn shortest_path(&self, s: &T, t: &T) {
        todo!()
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
        for edge in edges {
            let new: Rc<CommitHash> = graph.get_or_add_commit(&edge.commit);
            if !visited.contains(&new) {
                refs_stack.push(new.clone());
            }
            graph.add_edge(&old, &new, edge.cost.clone());
        }
        visited.insert(top);
    }
    graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    type TestHash = String;

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
    fn test_dijkstra_from_s() {
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
    fn test_dijkstra_from_t() {
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
    fn test_dijkstra_from_isolated_node() {
        let mut graph = CommitGraph::<TestHash> {
            commits: HashMap::new(),
            adj_list: HashMap::new(),
        };

        graph.add_commit("isolated_node".into());

        let paths = graph.dijkstra(Rc::new("ISO".into()), |ec| ec.patch);
        assert!(paths.is_empty());
    }
}
