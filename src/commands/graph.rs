use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
    rc::Rc,
};

use bincode::{Decode, Encode};

use crate::{
    object::{INDEX_HASH, Object, ObjectHash as CommitHash, commit::Commit, index::Index},
    storage::{StorageBackend, WrappedStorageBackend},
};

type Cost = u32;
#[derive(Debug, Clone, Encode, Decode)]
pub struct EdgeCost {
    pub patch: Cost,
    pub revert: Cost,
}

pub struct CommitGraph {
    commits: HashMap<Rc<CommitHash>, ()>,

    // should be a DAG
    adj_list: HashMap<Rc<CommitHash>, HashMap<Rc<CommitHash>, EdgeCost>>,
}

impl CommitGraph {
    pub fn add_commit(&mut self, commit: CommitHash) -> Rc<CommitHash> {
        if let Some(existing) = self.commits.keys().find(|rc| ***rc == commit).cloned() {
            return existing;
        }
        let rc = Rc::new(commit);
        self.commits.insert(rc.clone(), ());
        rc
    }
    pub fn get_or_add_commit(&mut self, commit: &CommitHash) -> Rc<CommitHash> {
        self.commits
            .keys()
            .find(|rc| ***rc == *commit)
            .cloned()
            .unwrap_or_else(|| self.add_commit(commit.clone()))
    }
    pub fn add_edge(&mut self, old: &CommitHash, new: &CommitHash, cost: EdgeCost) {
        let old_rc = self.get_or_add_commit(old);
        let new_rc = self.get_or_add_commit(new);
        self.adj_list
            .entry(new_rc)
            .or_default()
            .insert(old_rc, cost);
    }
    fn dijkstra(
        &self,
        s: Rc<CommitHash>,
        w: impl Fn(&EdgeCost) -> Cost,
    ) -> HashMap<Rc<CommitHash>, (Cost, Rc<CommitHash>)> {
        // todo: use heap to be more efficiently
        let mut done_map = HashMap::new();
        let mut todo_map = HashMap::new();
        todo_map.insert(s.clone(), (0, s.clone()));

        // get commit with min cost
        while let Some((commit, (cost, _prev))) = todo_map.iter().min_by_key(|e| e.0) {
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
    pub fn shortest_path(&self, s: &CommitHash, t: &CommitHash) {
        todo!()
    }
}

pub fn graph(backend: &WrappedStorageBackend) -> CommitGraph {
    let index = backend.get(INDEX_HASH).unwrap();
    let index = Index::deserialize(&index);
    let mut graph: CommitGraph = CommitGraph {
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
