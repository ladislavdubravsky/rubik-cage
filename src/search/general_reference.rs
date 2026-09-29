//! Frozen step-3 baseline for the opt-in search audit.
//! Resumable reference search over full, color-aware core positions.
//!
//! A horizon proves only whether a player can force a win within that many
//! plies. A failed horizon is never a draw. Exact distances require matching
//! upper/lower bounds; draws require insufficient material or a closed graph.
use super::{Evaluation, EvaluationMap};
use crate::core::{game::GameState, position::PositionKey};
use std::{cell::Cell, collections::HashMap};

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub max_steps: usize,
    pub max_records: usize,
    /// Conservative accounting for hash-table slack and suspended frames.
    pub max_bytes: usize,
    pub max_horizon: u16,
}

#[derive(Clone, Copy, Debug, Default)]
struct Bounds {
    lower: u16,
    upper: Option<u16>,
}

/// Earliest possible win, counting drops separately for each exact color.
fn earliest(state: &GameState, target: u8) -> Option<u16> {
    let totals = state.inventories();
    state
        .colors_for(target)
        .filter_map(|color| {
            let total = totals[color as usize];
            if total < 3 {
                return None;
            }
            let on_board = total - state.remaining(color);
            let drops = 3u16.saturating_sub(u16::from(on_board));
            Some(if drops == 0 {
                1
            } else {
                2 * drops - u16::from(state.player_to_move.id == target)
            })
        })
        .min()
}

pub(crate) fn children(key: PositionKey) -> Vec<PositionKey> {
    let state = key.to_state();
    let mut children = Vec::new();
    for m in state.legal_moves() {
        let mut child = state;
        child.apply_move(m).unwrap();
        children.push(child.position_key());
    }
    children.sort_unstable();
    children.dedup();
    // Terminal moves first. This changes work order, never the proof rules.
    children.sort_by_key(|key| match Evaluation::terminal(&key.to_state()) {
        Some(Evaluation::Win { winner, .. }) if winner == state.player_to_move.id => 0,
        Some(_) => 1,
        None => 2,
    });
    children
}

#[derive(Default)]
pub struct Proof {
    pub calls: Cell<u64>,
    pub hits: Cell<u64>,
    pub expansions: Cell<u64>,
    bounds: HashMap<(PositionKey, u8), Bounds>,
}
impl Proof {
    fn base(&self, key: PositionKey, target: u8, horizon: u16) -> Option<bool> {
        self.calls.set(self.calls.get() + 1);
        let state = key.to_state();
        if let Some(value) = Evaluation::terminal(&state) {
            return Some(value.winner() == Some(target));
        }
        match earliest(&state, target) {
            None => return Some(false),
            Some(n) if n > horizon => return Some(false),
            _ => (),
        }
        let result = self.bounds.get(&(key, target)).and_then(|b| {
            if b.upper.is_some_and(|n| n <= horizon) {
                Some(true)
            } else if b.lower > horizon {
                Some(false)
            } else {
                None
            }
        });
        if result.is_some() {
            self.hits.set(self.hits.get() + 1);
        }
        result
    }
    pub fn len(&self) -> usize {
        self.bounds.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }
    pub fn exact(&self, key: PositionKey) -> Option<Evaluation> {
        let state = key.to_state();
        if let Some(value) = Evaluation::terminal(&state) {
            return Some(value);
        }
        if earliest(&state, 0).is_none() && earliest(&state, 1).is_none() {
            return Some(Evaluation::Draw);
        }
        for target in 0..2 {
            if let Some(b) = self.bounds.get(&(key, target))
                && b.upper == Some(b.lower)
            {
                return Some(Evaluation::Win {
                    winner: target,
                    plies: u32::from(b.lower),
                });
            }
        }
        None
    }
    /// Independent local proof equations over ordinary core successors. Every
    /// dependency has a smaller horizon, so cyclic claims cannot justify wins.
    pub fn verify(&self) -> Result<(), String> {
        for (&(key, target), b) in &self.bounds {
            key.validate().map_err(str::to_owned)?;
            if target > 1 || b.upper.is_some_and(|n| n < b.lower) {
                return Err("Invalid general bound".into());
            }
            for (horizon, expected) in b
                .upper
                .map(|n| (n, true))
                .into_iter()
                .chain(b.lower.checked_sub(1).map(|n| (n, false)))
            {
                let state = key.to_state();
                let intrinsic = if let Some(value) = Evaluation::terminal(&state) {
                    Some(value.winner() == Some(target))
                } else if earliest(&state, target).is_none_or(|n| n > horizon) {
                    Some(false)
                } else {
                    None
                };
                let valid = if let Some(value) = intrinsic {
                    value == expected
                } else if horizon == 0 {
                    !expected
                } else {
                    let cs = children(key);
                    let answers: Vec<_> = cs
                        .iter()
                        .map(|&c| self.base(c, target, horizon - 1))
                        .collect();
                    // OR for the target, AND for the opponent.
                    if (key.turn() == target) == expected {
                        answers.contains(&Some(expected))
                    } else {
                        !answers.is_empty() && answers.iter().all(|v| *v == Some(expected))
                    }
                };
                if !valid {
                    return Err(format!(
                        "Unsupported bound: player {target}, horizon {horizon}"
                    ));
                }
            }
        }
        Ok(())
    }
}

struct Frame {
    key: PositionKey,
    horizon: u16,
    children: Option<Vec<PositionKey>>,
    next: usize,
}
impl Frame {
    fn new(key: PositionKey, horizon: u16) -> Self {
        Self {
            key,
            horizon,
            children: None,
            next: 0,
        }
    }
}
struct Query {
    target: u8,
    stack: Vec<Frame>,
    returned: Option<bool>,
}
impl Query {
    /// One expansion or one return edge; unfinished ancestors remain suspended.
    fn step(&mut self, proof: &mut Proof, max_records: usize) -> Result<(), ()> {
        let Some(frame) = self.stack.last_mut() else {
            return Ok(());
        };
        if frame.children.is_none() {
            if let Some(value) = proof.base(frame.key, self.target, frame.horizon) {
                self.stack.pop();
                self.returned = Some(value);
                return Ok(());
            }
            proof.expansions.set(proof.expansions.get() + 1);
            frame.children = Some(children(frame.key));
            return Ok(());
        }
        let cs = frame.children.as_ref().unwrap();
        let own_turn = frame.key.turn() == self.target;
        let finished = self
            .returned
            .filter(|v| *v == own_turn)
            .or_else(|| (frame.next == cs.len()).then_some(!own_turn));
        if let Some(value) = finished {
            let claim = (frame.key, self.target);
            if !proof.bounds.contains_key(&claim) && proof.len() >= max_records {
                return Err(());
            }
            let bound = proof.bounds.entry(claim).or_insert_with(|| Bounds {
                lower: earliest(&frame.key.to_state(), self.target).unwrap_or(u16::MAX),
                upper: None,
            });
            if value {
                bound.upper = Some(bound.upper.unwrap_or(u16::MAX).min(frame.horizon));
            } else {
                bound.lower = bound.lower.max(frame.horizon + 1);
            }
            self.stack.pop();
            self.returned = Some(value);
        } else {
            self.returned = None;
            let child = cs[frame.next];
            frame.next += 1;
            let horizon = frame.horizon - 1;
            self.stack.push(Frame::new(child, horizon));
        }
        Ok(())
    }
}
struct Task {
    key: PositionKey,
    horizon: u16,
    target_index: u8,
    query: Option<Query>,
}

pub struct Search {
    owners: [Option<u8>; 6],
    totals: [u8; 6],
    root: Option<PositionKey>,
    tasks: Vec<Task>,
    cursor: usize,
    pub proof: Proof,
    graph: Option<super::general_graph::Graph>,
    pub steps: usize,
}
pub struct Batch {
    pub values: EvaluationMap,
    pub complete: bool,
    pub at_capacity: bool,
    pub exhausted: bool,
}
impl Search {
    pub fn new(state: &GameState) -> Result<Self, String> {
        state.validate().map_err(str::to_owned)?;
        Ok(Self {
            owners: state.color_owners,
            totals: state.inventories(),
            root: None,
            tasks: Vec::new(),
            cursor: 0,
            proof: Proof::default(),
            graph: None,
            steps: 0,
        })
    }
    pub fn matches(&self, state: &GameState) -> bool {
        self.owners == state.color_owners && self.totals == state.inventories()
    }
    pub fn estimated_bytes(&self) -> usize {
        // Vec capacity and HashMap load-factor/allocator slack are deliberately
        // overestimated; this is an application budget, not a heap measurement.
        self.proof.len() * 4 * std::mem::size_of::<((PositionKey, u8), Bounds)>()
            + self.tasks.capacity() * std::mem::size_of::<Task>()
            + self
                .tasks
                .iter()
                .map(|t| {
                    t.query.as_ref().map_or(0, |q| {
                        2 * (q.stack.capacity() * std::mem::size_of::<Frame>()
                            + q.stack
                                .iter()
                                .map(|f| {
                                    f.children.as_ref().map_or(0, |c| {
                                        c.capacity() * std::mem::size_of::<PositionKey>()
                                    })
                                })
                                .sum::<usize>())
                    })
                })
                .sum::<usize>()
            + self.graph.as_ref().map_or(0, |g| g.estimated_bytes())
    }
    pub fn run(&mut self, state: &GameState, budget: Budget) -> Result<Batch, String> {
        state.validate().map_err(str::to_owned)?;
        if !self.matches(state) {
            return Err("General search namespace mismatch".into());
        }
        let root = state.position_key();
        if self.root != Some(root) {
            let mut requested = children(root);
            if !requested.contains(&root) {
                requested.push(root);
            }
            self.tasks = requested
                .into_iter()
                .map(|key| Task {
                    key,
                    horizon: 0,
                    target_index: 0,
                    query: None,
                })
                .collect();
            self.cursor = 0;
            self.root = Some(root);
            // Exhaustive graphs are useful only for very small inventories.
            // The frontier survives interruptions and is never rebuilt on a cap.
            self.graph = (self.totals.iter().map(|&n| usize::from(n)).sum::<usize>() <= 5)
                .then(|| super::general_graph::Graph::new(root, 4096, 65536));
        }
        let mut at_capacity = false;
        let mut idle = 0;
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        #[cfg(target_arch = "wasm32")]
        let started = web_sys::js_sys::Date::now();
        for i in 0..budget.max_steps {
            #[cfg(not(target_arch = "wasm32"))]
            let timed_out = started.elapsed().as_millis() >= 40;
            #[cfg(target_arch = "wasm32")]
            let timed_out = web_sys::js_sys::Date::now() - started >= 40.0;
            if timed_out {
                break;
            }
            // Leave room for the next frame, bound, or one graph expansion.
            if self.estimated_bytes().saturating_add(128 * 1024) > budget.max_bytes {
                at_capacity = true;
                break;
            }
            if i % 4 == 0
                && let Some(graph) = self.graph.as_mut()
                && graph.pending()
            {
                graph.step();
                self.steps += 1;
                idle = 0;
                continue;
            }
            let index = self.cursor;
            self.cursor = (self.cursor + 1) % self.tasks.len();
            let task = &mut self.tasks[index];
            if self.proof.exact(task.key).is_some()
                || self.graph.as_ref().and_then(|g| g.get(task.key)).is_some()
                || task.horizon > budget.max_horizon.min(u16::MAX - 1)
            {
                idle += 1;
                if idle >= self.tasks.len() && self.graph.as_ref().is_none_or(|g| !g.pending()) {
                    break;
                }
                continue;
            }
            idle = 0;
            if task.query.is_none() {
                task.query = Some(Query {
                    target: task.key.turn() ^ task.target_index,
                    stack: vec![Frame::new(task.key, task.horizon)],
                    returned: None,
                });
            }
            let query = task.query.as_mut().unwrap();
            if query.step(&mut self.proof, budget.max_records).is_err() {
                at_capacity = true;
                break;
            }
            self.steps += 1;
            if query.stack.is_empty() {
                task.query = None;
                task.target_index ^= 1;
                if task.target_index == 0 {
                    task.horizon += 1;
                }
            }
        }
        let values: EvaluationMap = self
            .tasks
            .iter()
            .filter_map(|t| {
                self.proof
                    .exact(t.key)
                    .or_else(|| self.graph.as_ref().and_then(|g| g.get(t.key)))
                    .map(|v| (t.key, v))
            })
            .collect();
        let complete = values.len() == self.tasks.len();
        let exhausted = self
            .tasks
            .iter()
            .all(|t| values.contains_key(&t.key) || t.horizon > budget.max_horizon)
            && self.graph.as_ref().is_none_or(|g| !g.pending());
        Ok(Batch {
            values,
            complete,
            at_capacity,
            exhausted,
        })
    }
}
