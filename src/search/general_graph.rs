//! Incremental complete-graph reference solver. No value escapes before closure.
use super::{Evaluation, general::children};
use crate::core::position::PositionKey;
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};
struct Node {
    key: PositionKey,
    parents: Vec<usize>,
    remaining: usize,
    longest: u32,
    value: Option<Evaluation>,
}
pub(super) struct Graph {
    nodes: Vec<Node>,
    ids: HashMap<PositionKey, usize>,
    next: usize,
    edges: usize,
    max_states: usize,
    max_edges: usize,
    queue: BinaryHeap<Reverse<(u32, usize)>>,
    active: Option<(u32, usize, usize)>,
    closed: bool,
    done: bool,
    abandoned: bool,
}
impl Graph {
    pub fn new(key: PositionKey, max_states: usize, max_edges: usize) -> Self {
        let mut result = Self {
            nodes: Vec::new(),
            ids: HashMap::new(),
            next: 0,
            edges: 0,
            max_states,
            max_edges,
            queue: BinaryHeap::new(),
            active: None,
            closed: false,
            done: false,
            abandoned: false,
        };
        result.insert(key);
        result
    }
    fn insert(&mut self, key: PositionKey) -> usize {
        let id = self.nodes.len();
        let value = Evaluation::terminal(&key.to_state());
        self.nodes.push(Node {
            key,
            parents: Vec::new(),
            remaining: 0,
            longest: 0,
            value,
        });
        self.ids.insert(key, id);
        if let Some(Evaluation::Win { plies, .. }) = value {
            self.queue.push(Reverse((plies, id)));
        }
        id
    }
    pub fn estimated_bytes(&self) -> usize {
        self.nodes.len()
            * 4
            * (std::mem::size_of::<Node>() + std::mem::size_of::<(PositionKey, usize)>() + 32)
            + self.edges * 2 * std::mem::size_of::<usize>()
    }
    pub fn pending(&self) -> bool {
        !self.done && !self.abandoned
    }
    pub fn get(&self, key: PositionKey) -> Option<Evaluation> {
        if !self.done {
            return None;
        }
        self.ids
            .get(&key)
            .map(|&id| self.nodes[id].value.unwrap_or(Evaluation::Draw))
    }
    pub fn step(&mut self) {
        if !self.pending() {
            return;
        }
        if !self.closed {
            if self.next == self.nodes.len() {
                self.closed = true;
                return;
            }
            let id = self.next;
            self.next += 1;
            if self.nodes[id].value.is_some() {
                return;
            }
            let cs = children(self.nodes[id].key);
            let new = cs.iter().filter(|k| !self.ids.contains_key(k)).count();
            if self.nodes.len() + new > self.max_states || self.edges + cs.len() > self.max_edges {
                self.abandoned = true;
                return;
            }
            self.edges += cs.len();
            self.nodes[id].remaining = cs.len();
            for key in cs {
                let child = self
                    .ids
                    .get(&key)
                    .copied()
                    .unwrap_or_else(|| self.insert(key));
                self.nodes[child].parents.push(id);
            }
            return;
        }
        if self.active.is_none() {
            let Some(Reverse((distance, id))) = self.queue.pop() else {
                self.done = true;
                return;
            };
            self.active = Some((distance, id, 0));
        }
        let (distance, id, cursor) = self.active.unwrap();
        if cursor == self.nodes[id].parents.len() {
            self.active = None;
            return;
        }
        self.active = Some((distance, id, cursor + 1));
        let winner = self.nodes[id].value.unwrap().winner().unwrap();
        let parent = self.nodes[id].parents[cursor];
        let node = &mut self.nodes[parent];
        if node.value.is_some() {
            return;
        }
        node.remaining -= 1;
        node.longest = node.longest.max(distance);
        let plies = if node.key.turn() == winner {
            distance + 1
        } else if node.remaining == 0 {
            node.longest + 1
        } else {
            return;
        };
        node.value = Some(Evaluation::Win { winner, plies });
        self.queue.push(Reverse((plies, parent)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::game::GameState,
        search::{EvaluationMap, retrograde},
    };
    #[test]
    fn incremental_graph_matches_reference_and_never_publishes_before_closure() {
        for totals in [[3, 1, 0, 0, 0, 0], [2, 1, 0, 0, 1, 0], [0; 6]] {
            let state =
                GameState::with_colors(GameState::multicolor().color_owners, totals).unwrap();
            let reference =
                retrograde::solve(&state, &EvaluationMap::new(), retrograde::Limits::default())
                    .unwrap();
            let mut graph = Graph::new(state.position_key(), 100_000, 1_500_000);
            while graph.pending() {
                assert_eq!(graph.get(state.position_key()), None);
                graph.step();
            }
            assert!(graph.done);
            let values: EvaluationMap = graph
                .ids
                .keys()
                .map(|&key| (key, graph.get(key).unwrap()))
                .collect();
            assert_eq!(values, reference.values);
            retrograde::verify(&values).unwrap();
            let mut limited = Graph::new(state.position_key(), 1, 1);
            while limited.pending() {
                limited.step();
            }
            assert!(limited.abandoned);
            assert_eq!(limited.get(state.position_key()), None);
            let count = limited.nodes.len();
            limited.step();
            assert_eq!(limited.nodes.len(), count);
        }
    }
}
