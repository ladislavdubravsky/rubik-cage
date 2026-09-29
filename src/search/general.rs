//! Resumable exact search over compact color-aware positions.
//! Core transitions remain the independent proof-verification authority.
//!
//! A horizon proves only whether a player can force a win within that many
//! plies. A failed horizon is never a draw. Exact distances require matching
//! upper/lower bounds; draws require insufficient material or a closed graph.
use super::general_compact::{Position, Space};
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

pub struct Proof {
    space: Space,
    bounds: HashMap<u128, Bounds>,
    pub calls: Cell<u64>,
    pub hits: Cell<u64>,
    pub expansions: Cell<u64>,
}
fn claim(key: Position, target: u8) -> u128 {
    key.0 | (u128::from(target) << 76)
}
impl Proof {
    fn new(space: Space) -> Self {
        Self {
            space,
            bounds: HashMap::new(),
            calls: Cell::new(0),
            hits: Cell::new(0),
            expansions: Cell::new(0),
        }
    }
    fn base(&self, key: Position, target: u8, horizon: u16) -> Option<bool> {
        self.calls.set(self.calls.get() + 1);
        if let Some(value) = self.space.terminal(key) {
            return Some(value.winner() == Some(target));
        }
        if self.space.earliest(key, target).is_none_or(|n| n > horizon) {
            return Some(false);
        }
        let value = self.cached(key, target, horizon);
        if value.is_some() {
            self.hits.set(self.hits.get() + 1);
        }
        value
    }
    fn cached(&self, key: Position, target: u8, horizon: u16) -> Option<bool> {
        self.bounds.get(&claim(key, target)).and_then(|b| {
            if b.upper.is_some_and(|n| n <= horizon) {
                Some(true)
            } else if b.lower > horizon {
                Some(false)
            } else {
                None
            }
        })
    }
    pub fn len(&self) -> usize {
        self.bounds.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }
    pub fn exact(&self, key: PositionKey) -> Option<Evaluation> {
        let key = self.space.try_encode(&key.to_state()).ok()?;
        self.exact_position(key)
    }
    fn exact_position(&self, key: Position) -> Option<Evaluation> {
        if let Some(value) = self.space.terminal(key) {
            return Some(value);
        }
        if self.space.earliest(key, 0).is_none() && self.space.earliest(key, 1).is_none() {
            return Some(Evaluation::Draw);
        }
        for target in 0..2 {
            if let Some(b) = self.bounds.get(&claim(key, target))
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
    /// Verify via ordinary core moves/outcomes/material, not compact transitions
    /// or compact terminal detection. Only dependency lookup uses compact keys.
    pub fn verify(&self) -> Result<(), String> {
        for (&encoded, b) in &self.bounds {
            let target = (encoded >> 76) as u8;
            let key = Position(encoded & ((1 << 76) - 1));
            if encoded >> 77 != 0 || target > 1 || b.upper.is_some_and(|n| n < b.lower) {
                return Err("Invalid general bound".into());
            }
            // Keys are private, constructed only by the checked compact space.
            let state = self.space.decode(key);
            state.validate().map_err(str::to_owned)?;
            if self.space.try_encode(&state).map_err(str::to_owned)? != key {
                return Err("Noncanonical general bound".into());
            }
            for (horizon, expected) in b
                .upper
                .map(|n| (n, true))
                .into_iter()
                .chain(b.lower.checked_sub(1).map(|n| (n, false)))
            {
                let intrinsic = core_base(&state, target, horizon);
                let valid = if let Some(value) = intrinsic {
                    value == expected
                } else if horizon == 0 {
                    !expected
                } else {
                    let answers: Vec<_> = children(state.position_key())
                        .iter()
                        .map(|c| {
                            let child = c.to_state();
                            core_base(&child, target, horizon - 1).or_else(|| {
                                self.cached(self.space.encode(&child), target, horizon - 1)
                            })
                        })
                        .collect();
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
fn core_base(state: &GameState, target: u8, horizon: u16) -> Option<bool> {
    if let Some(value) = Evaluation::terminal(state) {
        Some(value.winner() == Some(target))
    } else if earliest(state, target).is_none_or(|n| n > horizon) {
        Some(false)
    } else {
        None
    }
}

struct Frame {
    key: Position,
    horizon: u16,
    children: Option<Vec<Position>>,
    next: usize,
}
impl Frame {
    fn new(key: Position, horizon: u16) -> Self {
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
            frame.children = Some(proof.space.children(frame.key));
            return Ok(());
        }
        let cs = frame.children.as_ref().unwrap();
        let own_turn = frame.key.turn() == self.target;
        let finished = self
            .returned
            .filter(|v| *v == own_turn)
            .or_else(|| (frame.next == cs.len()).then_some(!own_turn));
        if let Some(value) = finished {
            let claim = claim(frame.key, self.target);
            if !proof.bounds.contains_key(&claim) && proof.len() >= max_records {
                return Err(());
            }
            let bound = proof.bounds.entry(claim).or_insert_with(|| Bounds {
                lower: proof
                    .space
                    .earliest(frame.key, self.target)
                    .unwrap_or(u16::MAX),
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
    position: Position,
    horizon: u16,
    target_index: u8,
    query: Option<Query>,
}

pub struct Search {
    owners: [Option<u8>; 6],
    totals: [u8; 6],
    root: Option<PositionKey>,
    tasks: Vec<Task>,
    requested: Vec<(PositionKey, Position)>,
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
        Self::with_options(state, true, true)
    }
    /// Switches for reproducible ablation audits; namespaces never change in place.
    pub fn with_options(state: &GameState, relabel: bool, ordering: bool) -> Result<Self, String> {
        let mut space = Space::new(state).map_err(str::to_owned)?;
        space.relabel = relabel;
        space.ordering = ordering;
        Ok(Self {
            owners: state.color_owners,
            totals: state.inventories(),
            root: None,
            tasks: Vec::new(),
            requested: Vec::new(),
            cursor: 0,
            proof: Proof::new(space),
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
        self.proof.len() * 4 * std::mem::size_of::<(u128, Bounds)>()
            + self.tasks.capacity() * std::mem::size_of::<Task>()
            + self.requested.capacity() * std::mem::size_of::<(PositionKey, Position)>()
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
                                        c.capacity() * std::mem::size_of::<Position>()
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
            self.requested = requested
                .into_iter()
                .map(|key| (key, self.proof.space.encode(&key.to_state())))
                .collect();
            self.tasks.clear();
            for &(key, position) in &self.requested {
                if !self.tasks.iter().any(|t| t.position == position) {
                    self.tasks.push(Task {
                        key,
                        position,
                        horizon: 0,
                        target_index: 0,
                        query: None,
                    });
                }
            }
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
            if self.proof.exact_position(task.position).is_some()
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
                    stack: vec![Frame::new(task.position, task.horizon)],
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
            .requested
            .iter()
            .filter_map(|&(key, position)| {
                self.proof
                    .exact_position(position)
                    .or_else(|| self.graph.as_ref().and_then(|g| g.get(key)))
                    .map(|v| (key, v))
            })
            .collect();
        let complete = values.len() == self.requested.len();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::cubie::Cubie, search::retrograde};
    fn budget(steps: usize) -> Budget {
        Budget {
            max_steps: steps,
            max_records: 100_000,
            max_bytes: 64 * 1024 * 1024,
            max_horizon: 10,
        }
    }
    fn multi(totals: [u8; 6]) -> GameState {
        GameState::with_colors(GameState::multicolor().color_owners, totals).unwrap()
    }
    #[test]
    fn material_bounds_never_combine_colors() {
        let state = multi([2, 2, 2, 2, 2, 2]);
        assert_eq!(earliest(&state, 0), None);
        assert_eq!(earliest(&state, 1), None);
        assert_eq!(
            Proof::new(Space::new(&state).unwrap()).exact(state.position_key()),
            Some(Evaluation::Draw)
        );
        let mut state = multi([3, 3, 0, 0, 3, 0]);
        state.cage.drop(Cubie::Blue, (0, 0)).unwrap();
        state.remaining_cubies[Cubie::Blue as usize] -= 1;
        assert_eq!(earliest(&state, 0), Some(3));
        assert_eq!(earliest(&state, 1), Some(6));
    }
    #[test]
    fn interrupted_horizon_search_agrees_with_complete_graphs() {
        // Include the old single-color representation and genuinely multiple
        // colors per player. Disable the graph so this exercises horizon proofs.
        for state in [
            GameState::new(3, 0),
            multi([3, 0, 0, 0, 1, 0]),
            multi([0, 3, 1, 0, 0, 0]),
        ] {
            let reference =
                retrograde::solve(&state, &EvaluationMap::new(), retrograde::Limits::default())
                    .unwrap();
            let mut search = Search::new(&state).unwrap();
            search.run(&state, budget(0)).unwrap();
            search.graph = None;
            let mut found = EvaluationMap::new();
            for _ in 0..100_000 {
                let result = search.run(&state, budget(1)).unwrap();
                for (key, value) in &result.values {
                    assert_eq!(Some(value), reference.values.get(key));
                }
                found.extend(result.values);
                if result.complete || result.exhausted {
                    break;
                }
            }
            search.proof.verify().unwrap();
            assert_eq!(
                found.get(&state.position_key()),
                reference.values.get(&state.position_key())
            );
            assert!(search.steps > 1);
        }
    }
    #[test]
    fn ablation_modes_preserve_exact_distances_and_public_color_keys() {
        let state = multi([3, 0, 0, 0, 1, 0]);
        let reference =
            retrograde::solve(&state, &EvaluationMap::new(), Default::default()).unwrap();
        for (relabel, ordering) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut search = Search::with_options(&state, relabel, ordering).unwrap();
            let mut final_values = EvaluationMap::new();
            for _ in 0..500 {
                let result = search.run(&state, budget(4000)).unwrap();
                for (key, value) in &result.values {
                    assert_eq!(reference.values.get(key), Some(value));
                }
                final_values.extend(result.values);
                if result.complete {
                    break;
                }
            }
            for child in children(state.position_key()) {
                assert!(final_values.contains_key(&child));
            }
            search.proof.verify().unwrap();
        }
        let drawn = multi([2; 6]);
        let mut search = Search::new(&drawn).unwrap();
        let result = search.run(&drawn, budget(0)).unwrap();
        assert!(result.complete);
        assert!(search.tasks.len() < search.requested.len());
        for m in drawn.legal_moves() {
            let mut child = drawn;
            child.apply_move(m).unwrap();
            assert_eq!(
                result.values.get(&child.position_key()),
                Some(&Evaluation::Draw)
            );
        }
    }

    #[test]
    fn finite_horizons_and_resource_limits_leave_unknown_and_resume() {
        let state = GameState::multicolor();
        let mut search = Search::new(&state).unwrap();
        let mut b = budget(1000);
        b.max_horizon = 0;
        let result = search.run(&state, b).unwrap();
        assert!(result.values.is_empty());
        assert!(!result.complete);
        assert!(result.exhausted);
        b.max_horizon = 6;
        b.max_bytes = 0;
        assert!(search.run(&state, b).unwrap().at_capacity);
        let before = search.steps;
        let result = search.run(&state, budget(100)).unwrap();
        assert!(search.steps > before);
        assert!(result.values.is_empty());
        search.proof.verify().unwrap();
        let mut changed = state;
        changed.remaining_cubies[0] -= 1;
        assert!(!search.matches(&changed));
        assert!(search.run(&changed, budget(1)).is_err());
        changed = state;
        changed.color_owners.swap(0, 1);
        assert!(!search.matches(&changed));
    }
    #[test]
    fn partial_results_include_tactics_and_proof_corruption_is_rejected() {
        let mut state = GameState::multicolor();
        for x in 0..2 {
            state.cage.drop(Cubie::Green, (x, 0)).unwrap();
            state.remaining_cubies[5] -= 1;
        }
        let mut search = Search::new(&state).unwrap();
        let mut result = search.run(&state, budget(0)).unwrap();
        assert!(result.values.values().any(|v| *v
            == Evaluation::Win {
                winner: 0,
                plies: 0
            }));
        for _ in 0..200 {
            result = search.run(&state, budget(100)).unwrap();
            if result.values.contains_key(&state.position_key()) {
                break;
            }
        }
        assert_eq!(
            result.values[&state.position_key()],
            Evaluation::Win {
                winner: 0,
                plies: 1
            }
        );
        assert!(!result.complete);
        search.proof.verify().unwrap();
        search.proof.bounds.insert(
            claim(search.proof.space.encode(&GameState::multicolor()), 0),
            Bounds {
                lower: 1,
                upper: Some(1),
            },
        );
        assert!(search.proof.verify().is_err());
    }
    #[test]
    fn record_limit_preserves_suspended_queries_and_completed_bounds() {
        let state = multi([3, 0, 0, 0, 1, 0]);
        let mut search = Search::new(&state).unwrap();
        search.run(&state, budget(0)).unwrap();
        search.graph = None;
        let mut tiny = budget(10_000);
        tiny.max_records = 1;
        for _ in 0..100 {
            if search.run(&state, tiny).unwrap().at_capacity {
                break;
            }
        }
        assert_eq!(search.proof.len(), 1);
        search.proof.verify().unwrap();
        let first = *search.proof.bounds.keys().next().unwrap();
        for _ in 0..1000 {
            let result = search.run(&state, budget(1000)).unwrap();
            if result.values.contains_key(&state.position_key()) {
                break;
            }
        }
        assert!(search.proof.bounds.contains_key(&first));
        assert!(search.proof.exact(state.position_key()).is_some());
        search.proof.verify().unwrap();
    }
}
