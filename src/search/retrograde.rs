//! Distance-ordered retrograde analysis of a complete reachable game graph.
//! Adapted from the preserved WIP; all lookups now use complete canonical states.
use super::{Evaluation, EvaluationMap};
use crate::core::game::GameState;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Limits {
    pub max_states: usize,
    pub max_edges: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_states: 100_000,
            max_edges: 1_500_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub states: usize,
    pub edges: usize,
    pub boundaries: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolveError {
    InvalidState(String),
    LimitReached(Stats),
    DistanceOverflow,
    ConflictingTerminal,
}
impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SolveError {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Solution {
    pub values: EvaluationMap,
    pub stats: Stats,
}

struct Node {
    owner: u8,
    children: Vec<usize>,
    seed: Option<Evaluation>,
}

/// `known` must contain only compatible, certified exact values. Cached boundaries
/// keep their original distances. On a resource limit no provisional table escapes.
pub fn solve(
    state: &GameState,
    known: &EvaluationMap,
    limits: Limits,
) -> Result<Solution, SolveError> {
    state
        .validate()
        .map_err(|e| SolveError::InvalidState(e.into()))?;
    if limits.max_states == 0 {
        return Err(SolveError::LimitReached(Stats::default()));
    }
    let root = state.position_key();
    let mut keys = vec![root];
    let mut ids = HashMap::from([(root, 0)]);
    let mut nodes = Vec::new();
    let mut stats = Stats::default();
    let mut index = 0;
    while index < keys.len() {
        let key = keys[index];
        let state = key.to_state();
        let terminal = Evaluation::terminal(&state);
        let cached = known.get(&key).copied();
        if terminal.is_some() && cached.is_some() && terminal != cached {
            return Err(SolveError::ConflictingTerminal);
        }
        let seed = terminal.or(cached);
        let mut children = Vec::new();
        if seed.is_none() {
            for m in state.legal_moves() {
                let mut child = state;
                child.apply_move(m).expect("Generated legal move");
                let child_key = child.position_key();
                let id = if let Some(&id) = ids.get(&child_key) {
                    id
                } else {
                    if keys.len() >= limits.max_states {
                        stats.states = keys.len();
                        return Err(SolveError::LimitReached(stats));
                    }
                    let id = keys.len();
                    ids.insert(child_key, id);
                    keys.push(child_key);
                    id
                };
                if !children.contains(&id) {
                    if stats.edges >= limits.max_edges {
                        stats.states = keys.len();
                        return Err(SolveError::LimitReached(stats));
                    }
                    children.push(id);
                    stats.edges += 1;
                }
            }
        } else if terminal.is_none() {
            stats.boundaries += 1;
        }
        nodes.push(Node {
            owner: key.turn(),
            children,
            seed,
        });
        index += 1;
    }
    stats.states = nodes.len();
    let values = propagate(&nodes)?;
    Ok(Solution {
        values: keys.into_iter().zip(values).collect(),
        stats,
    })
}

fn propagate(nodes: &[Node]) -> Result<Vec<Evaluation>, SolveError> {
    let mut predecessors = vec![Vec::new(); nodes.len()];
    for (id, node) in nodes.iter().enumerate() {
        for &child in &node.children {
            predecessors[child].push(id);
        }
    }
    let mut values: Vec<_> = nodes.iter().map(|n| n.seed).collect();
    let mut remaining: Vec<_> = nodes.iter().map(|n| n.children.len()).collect();
    let mut max_distance = vec![0; nodes.len()];
    let mut queue = BinaryHeap::new();
    for (id, value) in values.iter().enumerate() {
        if let Some(Evaluation::Win { plies, .. }) = value {
            queue.push(Reverse((*plies, id)));
        }
    }
    while let Some(Reverse((distance, id))) = queue.pop() {
        let winner = values[id].unwrap().winner().unwrap();
        for &parent in &predecessors[id] {
            if values[parent].is_some() {
                continue;
            }
            let next_distance = if nodes[parent].owner == winner {
                Some(distance)
            } else {
                remaining[parent] -= 1;
                max_distance[parent] = max_distance[parent].max(distance);
                (remaining[parent] == 0).then_some(max_distance[parent])
            };
            if let Some(d) = next_distance {
                let plies = d.checked_add(1).ok_or(SolveError::DistanceOverflow)?;
                values[parent] = Some(Evaluation::Win { winner, plies });
                queue.push(Reverse((plies, parent)));
            }
        }
    }
    // Only safe now: construction included every successor of every unseeded node.
    Ok(values
        .into_iter()
        .map(|v| v.unwrap_or(Evaluation::Draw))
        .collect())
}

/// Validate a full table independently of graph construction and queue propagation.
/// Positive decisive distances plus these equations certify forced termination.
pub fn verify(values: &EvaluationMap) -> Result<(), String> {
    for (key, &value) in values {
        key.validate().map_err(str::to_owned)?;
        let state = key.to_state();
        if let Some(terminal) = Evaluation::terminal(&state) {
            if terminal != value {
                return Err(format!("Incorrect terminal value: {value:?}"));
            }
            continue;
        }
        let mut children = Vec::new();
        for m in state.legal_moves() {
            let mut child = state;
            child.apply_move(m).unwrap();
            let child_key = child.position_key();
            let child_value = values
                .get(&child_key)
                .copied()
                .or_else(|| Evaluation::terminal(&child))
                .ok_or_else(|| format!("Missing successor for {key:?}"))?;
            children.push(child_value);
        }
        let owner = state.player_to_move.id;
        let own_win = children
            .iter()
            .filter(|v| v.winner() == Some(owner))
            .filter_map(|v| v.plies())
            .min();
        let expected = if let Some(d) = own_win {
            Evaluation::Win {
                winner: owner,
                plies: d.checked_add(1).ok_or("Distance overflow")?,
            }
        } else if !children.is_empty() && children.iter().all(|v| v.winner() == Some(1 - owner)) {
            let d = children.iter().filter_map(|v| v.plies()).max().unwrap();
            Evaluation::Win {
                winner: 1 - owner,
                plies: d.checked_add(1).ok_or("Distance overflow")?,
            }
        } else {
            Evaluation::Draw
        };
        if value != expected {
            return Err(format!(
                "Minimax equation failed for {key:?}: {value:?} != {expected:?}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "retrograde_tests.rs"]
mod tests;
