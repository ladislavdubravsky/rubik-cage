//! Bounded approximate play. No score or transposition entry enters exact caches.
//!
//! An explicit stack retains interrupted alpha-beta iterations. Repetitions
//! consume depth (and at most two tactical extension plies), never imply Draw.
//! Exact inputs are sampled only at the root and its legal children, so starting
//! an analysis never clones or canonicalizes a large supplied evaluation table.
use super::{
    Evaluation, EvaluationMap,
    general_compact::{Position, Space},
    lookup_exact,
};
use crate::core::{
    game::{GameState, RULES_VERSION},
    r#move::Move,
    position::PositionKey,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const EVALUATOR_VERSION: u32 = 1;
pub const MATE_SCORE: i32 = 1_000_000;
pub const TACTICAL_EXTENSIONS: u8 = 2;
const INFINITY: i32 = 2_000_000;
const MATE_THRESHOLD: i32 = MATE_SCORE / 2;
const MAX_PV: usize = 12;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_depth: u16,
    pub max_records: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 6,
            max_records: 25_000,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Maximum elementary search steps, including tactical probes and returns.
    pub max_nodes: u64,
    /// Cooperative time target, checked between bounded steps.
    pub max_millis: u32,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            max_nodes: 4000,
            max_millis: 40,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub position: PositionKey,
    pub best_move: Option<Move>,
    pub completed_depth: u16,
    /// Cumulative elementary search steps, not a count of unique positions.
    pub nodes: u64,
    /// Root player's perspective. A mate-scale score is still an analysis score.
    pub score: i32,
    /// Legal moves in the original coordinates/colors, capped at twelve moves.
    pub principal_variation: Vec<Move>,
    pub tactical_complete: bool,
    pub finished: bool,
    /// Only the independently supplied/terminal/material root value.
    pub exact: Option<Evaluation>,
    pub rules_version: u32,
    pub evaluator_version: u32,
}

/// Static potential from the side to move's perspective, for valid game states.
/// Scores are ordinal heuristic units, not probabilities or exact game values.
pub fn heuristic(state: &GameState) -> i32 {
    let space = Space::new(state).expect("AI heuristic requires a valid position");
    space.heuristic(space.encode(state), state.player_to_move.id)
}

fn value_score(value: Evaluation, turn: u8, ply: u16) -> i32 {
    match value {
        Evaluation::Draw => 0,
        Evaluation::Win { winner, plies } => {
            // Keep even an unusually long exact mate outside heuristic scores.
            let distance = plies.saturating_add(u32::from(ply)).min(400_000) as i32;
            if winner == turn {
                MATE_SCORE - distance
            } else {
                -MATE_SCORE + distance
            }
        }
    }
}
fn normalize_score(score: i32, ply: u16) -> i32 {
    if score > MATE_THRESHOLD {
        score + i32::from(ply)
    } else if score < -MATE_THRESHOLD {
        score - i32::from(ply)
    } else {
        score
    }
}
fn denormalize_score(score: i32, ply: u16) -> i32 {
    if score > MATE_THRESHOLD {
        score - i32::from(ply)
    } else if score < -MATE_THRESHOLD {
        score + i32::from(ply)
    } else {
        score
    }
}

#[derive(Clone, Copy)]
enum Bound {
    ExactAtDepth,
    Lower,
    Upper,
}
#[derive(Clone, Copy)]
struct Entry {
    position: Position,
    depth: u16,
    extensions: u8,
    bound: Bound,
    score: i32,
    best: Option<Position>,
    age: u16,
}
struct Table {
    entries: Vec<Option<Entry>>,
}
impl Table {
    fn new(capacity: usize) -> Self {
        Self {
            entries: vec![None; capacity],
        }
    }
    fn slot(&self, position: Position) -> usize {
        let mut x = position.0 as u64 ^ ((position.0 >> 64) as u64).rotate_left(23);
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        ((x ^ (x >> 31)) % self.entries.len() as u64) as usize
    }
    fn get(&self, position: Position) -> Option<Entry> {
        if self.entries.is_empty() {
            return None;
        }
        self.entries[self.slot(position)].filter(|entry| entry.position == position)
    }
    fn put(&mut self, entry: Entry) {
        if self.entries.is_empty() {
            return;
        }
        let slot = self.slot(entry.position);
        if self.entries[slot].is_none_or(|old| old.age != entry.age || old.depth <= entry.depth) {
            self.entries[slot] = Some(entry);
        }
    }
}
struct RootMove {
    position: Position,
    exact: Option<Evaluation>,
    unsafe_now: bool,
}
struct Screen {
    index: usize,
    replies: Option<Vec<Position>>,
    next: usize,
}
struct Frame {
    position: Position,
    depth: u16,
    extensions: u8,
    ply: u16,
    alpha: i32,
    beta: i32,
    original_alpha: i32,
    original_beta: i32,
    children: Option<Vec<Position>>,
    next: usize,
    best: i32,
    pv: Vec<Position>,
}
impl Frame {
    fn new(
        position: Position,
        depth: u16,
        extensions: u8,
        ply: u16,
        alpha: i32,
        beta: i32,
    ) -> Self {
        Self {
            position,
            depth,
            extensions,
            ply,
            alpha,
            beta,
            original_alpha: alpha,
            original_beta: beta,
            children: None,
            next: 0,
            best: -INFINITY,
            pv: Vec::new(),
        }
    }
}
struct Returned {
    score: i32,
    pv: Vec<Position>,
}

pub struct Search {
    state: GameState,
    space: Space,
    position: Position,
    roots: Vec<RootMove>,
    allowed: Vec<Position>,
    known: HashMap<Position, Evaluation>,
    limits: Limits,
    table: Table,
    screen: Screen,
    stack: Vec<Frame>,
    returned: Option<Returned>,
    iteration: u16,
    analysis: Analysis,
}
impl Search {
    pub fn new(
        state: &GameState,
        known: &EvaluationMap,
        mut limits: Limits,
    ) -> Result<Self, String> {
        let space = Space::new(state).map_err(str::to_owned)?;
        // Limit stack/PV construction and prevent accidental unbounded allocation.
        limits.max_depth = limits.max_depth.min(64);
        limits.max_records = limits.max_records.min(1_000_000);
        let position = space.encode(state);
        let material_draw =
            space.earliest(position, 0).is_none() && space.earliest(position, 1).is_none();
        let exact =
            lookup_exact(known, state).or_else(|| material_draw.then_some(Evaluation::Draw));
        let mut exact_positions = HashMap::new();
        if let Some(value) = exact {
            exact_positions.insert(position, value);
        }
        let mut roots: Vec<RootMove> = Vec::new();
        for m in state.legal_moves() {
            let mut child = *state;
            child.apply_move(m).map_err(str::to_owned)?;
            let child_position = space.encode(&child);
            let value =
                lookup_exact(known, &child).or_else(|| material_draw.then_some(Evaluation::Draw));
            if let Some(value) = value {
                if exact_positions
                    .insert(child_position, value)
                    .is_some_and(|old| old != value)
                {
                    return Err("Conflicting equivalent exact inputs for AI".into());
                }
            }
            if let Some(existing) = roots.iter_mut().find(|r| r.position == child_position) {
                existing.exact = existing.exact.or(value);
            } else {
                roots.push(RootMove {
                    position: child_position,
                    exact: value,
                    unsafe_now: false,
                });
            }
        }
        let finished = roots.is_empty();
        let initial_score = exact.map_or_else(
            || space.heuristic(position, state.player_to_move.id),
            |v| value_score(v, state.player_to_move.id, 0),
        );
        let mut result = Self {
            state: *state,
            space,
            position,
            roots,
            allowed: Vec::new(),
            known: exact_positions,
            limits,
            table: Table::new(limits.max_records),
            screen: Screen {
                index: 0,
                replies: None,
                next: 0,
            },
            stack: Vec::new(),
            returned: None,
            iteration: 1,
            analysis: Analysis {
                position: state.position_key(),
                best_move: None,
                completed_depth: 0,
                nodes: 0,
                score: initial_score,
                principal_variation: Vec::new(),
                tactical_complete: finished,
                finished,
                exact,
                rules_version: RULES_VERSION,
                evaluator_version: EVALUATOR_VERSION,
            },
        };
        // An exact root and one matching child already identify an optimal
        // action, even when other move values are absent from the supplied map.
        // In particular, a proved draw must not lose to an optimistic unknown.
        let certified: Vec<_> = result
            .roots
            .iter()
            .filter(|r| match (exact, r.exact) {
                (Some(Evaluation::Draw), Some(Evaluation::Draw)) => true,
                (
                    Some(Evaluation::Win {
                        winner: a,
                        plies: n,
                    }),
                    Some(Evaluation::Win {
                        winner: b,
                        plies: m,
                    }),
                ) => a == b && m.checked_add(1) == Some(n),
                _ => false,
            })
            .map(|r| r.position)
            .collect();
        let certified_choice = !certified.is_empty();
        result.allowed = if certified_choice {
            certified
        } else {
            result.roots.iter().map(|r| r.position).collect()
        };
        result.choose_fallback();
        // Terminal wins were checked for every original legal move above. Exact
        // child values also settle selection without spending a heuristic horizon.
        if !finished
            && (certified_choice
                || result.roots.iter().all(|r| r.exact.is_some())
                || result.roots.iter().any(|r| {
                    r.exact
                        == Some(Evaluation::Win {
                            winner: state.player_to_move.id,
                            plies: 0,
                        })
                }))
        {
            result.analysis.tactical_complete = true;
            result.analysis.finished = true;
        }
        Ok(result)
    }

    /// Retain incomplete iterations across calls. The result always includes a
    /// legal fallback; wait for tactical_complete before automatic move execution.
    pub fn run(&mut self, budget: Budget) -> Analysis {
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        #[cfg(target_arch = "wasm32")]
        let started = web_sys::js_sys::Date::now();
        for _ in 0..budget.max_nodes {
            if self.analysis.finished {
                break;
            }
            #[cfg(not(target_arch = "wasm32"))]
            let elapsed = started.elapsed().as_millis() as u64;
            #[cfg(target_arch = "wasm32")]
            let elapsed = (web_sys::js_sys::Date::now() - started).max(0.0) as u64;
            if elapsed >= u64::from(budget.max_millis) {
                break;
            }
            self.analysis.nodes += 1;
            if !self.analysis.tactical_complete {
                self.screen_step();
            } else {
                self.search_step();
            }
        }
        self.analysis.clone()
    }

    fn root_score(&self, position: Position) -> i32 {
        if let Some(&value) = self.known.get(&position) {
            value_score(value, self.state.player_to_move.id, 1)
        } else {
            self.space.heuristic(position, self.state.player_to_move.id)
        }
    }
    fn choose_fallback(&mut self) {
        let Some(position) = self
            .allowed
            .iter()
            .copied()
            .max_by_key(|&p| self.root_score(p))
        else {
            return;
        };
        self.analysis.score = self.root_score(position);
        self.publish_pv(&[position]);
    }
    fn publish_pv(&mut self, positions: &[Position]) {
        let mut state = self.state;
        let mut pv = Vec::new();
        for &position in positions.iter().take(MAX_PV) {
            let Some((m, child)) = state.legal_moves().into_iter().find_map(|m| {
                let mut child = state;
                child.apply_move(m).ok()?;
                (self.space.encode(&child) == position).then_some((m, child))
            }) else {
                break;
            };
            pv.push(m);
            state = child;
        }
        if let Some(&m) = pv.first() {
            self.analysis.best_move = Some(m);
            self.analysis.principal_variation = pv;
        }
    }
    fn screen_step(&mut self) {
        if self.screen.index == self.roots.len() {
            let safe = self.roots.iter().any(|r| !r.unsafe_now);
            self.allowed = self
                .roots
                .iter()
                .filter(|r| !safe || !r.unsafe_now)
                .map(|r| r.position)
                .collect();
            self.choose_fallback();
            self.analysis.tactical_complete = true;
            if self.limits.max_depth == 0 {
                self.analysis.finished = true;
            }
            return;
        }
        let index = self.screen.index;
        let root = &mut self.roots[index];
        if let Some(value) = root.exact {
            root.unsafe_now = matches!(value, Evaluation::Win { winner, plies }
                if winner != self.state.player_to_move.id && plies <= 1);
            self.screen.index += 1;
            return;
        }
        if self.screen.replies.is_none() {
            self.screen.replies = Some(self.space.children(root.position));
            self.screen.next = 0;
            return;
        }
        let replies = self.screen.replies.as_ref().unwrap();
        if self.screen.next == replies.len() {
            self.screen.index += 1;
            self.screen.replies = None;
            return;
        }
        let reply = replies[self.screen.next];
        self.screen.next += 1;
        if self
            .space
            .terminal(reply)
            .is_some_and(|v| v.winner() == Some(1 - self.state.player_to_move.id))
        {
            root.unsafe_now = true;
            self.screen.index += 1;
            self.screen.replies = None;
        }
    }

    fn leaf_value(&self, position: Position, ply: u16) -> Option<i32> {
        self.space
            .terminal(position)
            .or_else(|| self.known.get(&position).copied())
            .or_else(|| {
                (self.space.earliest(position, 0).is_none()
                    && self.space.earliest(position, 1).is_none())
                .then_some(Evaluation::Draw)
            })
            .map(|v| value_score(v, position.turn(), ply))
    }
    fn return_leaf(&mut self, score: i32, pv: Vec<Position>) {
        self.stack.pop();
        self.returned = Some(Returned { score, pv });
    }
    fn finish_frame(&mut self) {
        let frame = self.stack.pop().unwrap();
        let bound = if frame.best <= frame.original_alpha {
            Bound::Upper
        } else if frame.best >= frame.original_beta {
            Bound::Lower
        } else {
            Bound::ExactAtDepth
        };
        self.table.put(Entry {
            position: frame.position,
            depth: frame.depth,
            extensions: frame.extensions,
            bound,
            score: normalize_score(frame.best, frame.ply),
            best: frame.pv.first().copied(),
            age: self.iteration,
        });
        self.returned = Some(Returned {
            score: frame.best,
            pv: frame.pv,
        });
    }
    fn search_step(&mut self) {
        if let Some(returned) = self.returned.take() {
            if self.stack.is_empty() {
                self.analysis.completed_depth = self.iteration;
                self.analysis.score = returned.score;
                self.publish_pv(&returned.pv);
                if self.iteration >= self.limits.max_depth {
                    self.analysis.finished = true;
                } else {
                    self.iteration += 1;
                }
                return;
            }
            let parent = self.stack.last_mut().unwrap();
            let score = -returned.score;
            if score > parent.best {
                parent.best = score;
                parent.pv.clear();
                parent
                    .pv
                    .push(parent.children.as_ref().unwrap()[parent.next - 1]);
                parent.pv.extend(returned.pv.into_iter().take(MAX_PV - 1));
            }
            parent.alpha = parent.alpha.max(score);
            if parent.alpha >= parent.beta || parent.next == parent.children.as_ref().unwrap().len()
            {
                self.finish_frame();
            }
            return;
        }
        if self.stack.is_empty() {
            self.stack.push(Frame::new(
                self.position,
                self.iteration,
                TACTICAL_EXTENSIONS,
                0,
                -INFINITY,
                INFINITY,
            ));
            return;
        }
        let index = self.stack.len() - 1;
        let position = self.stack[index].position;
        if self.stack[index].children.is_none() {
            let ply = self.stack[index].ply;
            if ply != 0 {
                if let Some(value) = self.leaf_value(position, ply) {
                    self.return_leaf(value, Vec::new());
                    return;
                }
            }
            let cached = self.table.get(position);
            if ply != 0 {
                if let Some(entry) = cached.filter(|e| {
                    e.depth == self.stack[index].depth
                        && e.extensions == self.stack[index].extensions
                }) {
                    let value = denormalize_score(entry.score, ply);
                    let frame = &mut self.stack[index];
                    match entry.bound {
                        Bound::ExactAtDepth => {
                            self.return_leaf(value, entry.best.into_iter().collect());
                            return;
                        }
                        Bound::Lower => frame.alpha = frame.alpha.max(value),
                        Bound::Upper => frame.beta = frame.beta.min(value),
                    }
                    if frame.alpha >= frame.beta {
                        self.return_leaf(value, entry.best.into_iter().collect());
                        return;
                    }
                    frame.original_alpha = frame.alpha;
                    frame.original_beta = frame.beta;
                }
            }
            let mut children = if ply == 0 {
                self.allowed.clone()
            } else {
                self.space.children(position)
            };
            // Every leaf includes this one-move tactical check, including moves
            // that win through gravity despite no pair in the current geometry.
            if let Some(&winning) = children.iter().find(|&&p| {
                self.space
                    .terminal(p)
                    .is_some_and(|v| v.winner() == Some(position.turn()))
            }) {
                self.return_leaf(MATE_SCORE - i32::from(ply) - 1, vec![winning]);
                return;
            }
            let frame = &self.stack[index];
            if frame.depth == 0 && (frame.extensions == 0 || !self.space.volatile(position)) {
                let value = self.space.heuristic(position, position.turn());
                self.return_leaf(value, Vec::new());
                return;
            }
            if children.is_empty() {
                // Valid nonterminal core positions have manipulation moves. This
                // fallback remains heuristic instead of inventing an exact draw.
                self.return_leaf(self.space.heuristic(position, position.turn()), Vec::new());
                return;
            }
            let turn = position.turn();
            children.sort_by_cached_key(|&p| {
                let preferred = cached.is_some_and(|e| e.best == Some(p));
                let value = self
                    .known
                    .get(&p)
                    .copied()
                    .or_else(|| self.space.terminal(p))
                    .map_or_else(
                        || self.space.heuristic(p, turn),
                        |v| value_score(v, turn, ply + 1),
                    );
                (std::cmp::Reverse(preferred), std::cmp::Reverse(value))
            });
            self.stack[index].children = Some(children);
            return;
        }
        let frame = self.stack.last_mut().unwrap();
        let child = frame.children.as_ref().unwrap()[frame.next];
        frame.next += 1;
        let depth = frame.depth.saturating_sub(1);
        let extensions = if frame.depth == 0 {
            frame.extensions - 1
        } else {
            frame.extensions
        };
        let next = Frame::new(
            child,
            depth,
            extensions,
            frame.ply + 1,
            -frame.beta,
            -frame.alpha,
        );
        self.stack.push(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mate_scores_survive_transposition_at_different_ply() {
        for score in [MATE_SCORE - 9, -MATE_SCORE + 9, 1200, -1200, 0] {
            assert_eq!(denormalize_score(normalize_score(score, 4), 4), score);
        }
    }
    #[test]
    fn collisions_and_mate_windows_preserve_completed_iteration_scores() {
        let tactical = crate::core::snapshot::decode(include_bytes!(
            "../../tests/fixtures/multicolor-exact/tactic-win-in-3.rcg"
        ))
        .unwrap();
        for (state, depth) in [(GameState::multicolor(), 3), (tactical, 2)] {
            let mut expected = None;
            for max_records in [0, 1, 64, 4096] {
                let mut search = Search::new(
                    &state,
                    &EvaluationMap::new(),
                    Limits {
                        max_depth: depth,
                        max_records,
                    },
                )
                .unwrap();
                let mut result = search.run(Budget {
                    max_nodes: 0,
                    max_millis: 0,
                });
                for _ in 0..1000 {
                    result = search.run(Budget {
                        max_nodes: 4000,
                        max_millis: 100,
                    });
                    if result.finished {
                        break;
                    }
                }
                assert!(result.finished);
                assert_eq!(result.completed_depth, depth);
                if let Some(score) = expected {
                    assert_eq!(result.score, score);
                }
                expected = Some(result.score);
                assert!(result.exact.is_none());
            }
        }
    }
    #[test]
    fn terminal_root_never_suggests_a_move() {
        let mut state = GameState::multicolor();
        for x in 0..3 {
            state.cage.grid[x][0][0] = Some(crate::core::cubie::Cubie::White);
        }
        state.remaining_cubies[0] = 0;
        let mut search = Search::new(&state, &EvaluationMap::new(), Limits::default()).unwrap();
        let result = search.run(Budget::default());
        assert!(result.finished);
        assert!(result.best_move.is_none());
        assert_eq!(result.exact, Evaluation::terminal(&state));
    }
}
