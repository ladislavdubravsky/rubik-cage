//! Goal-directed finite-horizon minimax. A failed query is only a lower bound on
//! distance, never a draw. Combining adjacent failed/successful horizons yields
//! exact distances. Closed safety strategies additionally certify draws. Every
//! published claim is independently checkable against core.
use super::{
    Evaluation, EvaluationMap,
    packed::{POSITION_MASK, Position, Space},
};
use crate::core::game::GameState;
use bincode::{Decode, Encode};
use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Bounds {
    /// No victory within not_within-1 plies; zero means no negative claim.
    pub not_within: u8,
    /// A forced victory within this many plies; 255 means no positive claim.
    pub within: u8,
}
impl Bounds {
    fn query(self, horizon: u8) -> Option<bool> {
        if self.within <= horizon {
            Some(true)
        } else if self.not_within > horizon {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Debug, Encode, Decode)]
pub struct Proof {
    magic: [u8; 8],
    rules: u32,
    pub space: Space,
    /// Bit 52 selects the player whose forced victory is being proved.
    pub bounds: HashMap<u64, Bounds>,
    /// Positions from which the selected target can be prevented from ever winning.
    /// These are closed safety strategies, not finite-horizon failures.
    pub safe: BTreeSet<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub max_positions: usize,
    pub max_calls: u64,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            max_positions: 10_000_000,
            max_calls: 500_000_000,
        }
    }
}

pub struct Search {
    pub proof: Proof,
    pub calls: u64,
    pub hits: u64,
    pub budget: Budget,
}
impl Search {
    pub fn new(state: &GameState, budget: Budget) -> Result<Self, String> {
        Ok(Self {
            proof: Proof {
                magic: *b"RCGPRF02",
                rules: crate::core::game::RULES_VERSION,
                space: Space::new(state).map_err(str::to_owned)?,
                bounds: HashMap::new(),
                safe: BTreeSet::new(),
            },
            calls: 0,
            hits: 0,
            budget,
        })
    }
    pub fn query(&mut self, pos: Position, target: u8, horizon: u8) -> Result<bool, String> {
        assert!(target < 2 && horizon < 255);
        self.calls += 1;
        if self.calls > self.budget.max_calls {
            return Err("Search call limit reached; unfinished queries remain unknown".into());
        }
        if let Some(winner) = pos.terminal() {
            return Ok(winner == target as i8);
        }
        if self
            .proof
            .safe
            .contains(&(pos.0 | (u64::from(target) << 52)))
        {
            return Ok(false);
        }
        let earliest = self.proof.space.earliest(pos, target);
        if horizon < earliest {
            return Ok(false);
        }
        let key = pos.0 | (u64::from(target) << 52);
        if let Some(result) = self.proof.bounds.get(&key).and_then(|b| b.query(horizon)) {
            self.hits += 1;
            return Ok(result);
        }
        if self.proof.bounds.len() >= self.budget.max_positions
            && !self.proof.bounds.contains_key(&key)
        {
            return Err("Search position limit reached; unfinished queries remain unknown".into());
        }
        let owner = pos.turn();
        let mut children = self.proof.space.children(pos);
        let mut result = owner != target;
        for &child in children.ordered(owner) {
            let child_wins = self.query(child, target, horizon - 1)?;
            if owner == target && child_wins {
                result = true;
                break;
            }
            if owner != target && !child_wins {
                result = false;
                break;
            }
        }
        // Descendants may have filled the table while this parent was open.
        if self.proof.bounds.len() >= self.budget.max_positions
            && !self.proof.bounds.contains_key(&key)
        {
            return Err("Search position limit reached; unfinished queries remain unknown".into());
        }
        let entry = self.proof.bounds.entry(key).or_insert(Bounds {
            not_within: earliest,
            within: 255,
        });
        if result {
            entry.within = entry.within.min(horizon);
        } else {
            entry.not_within = entry.not_within.max(horizon + 1);
        }
        assert!(
            entry.not_within <= entry.within,
            "Contradictory horizon bounds"
        );
        Ok(result)
    }
    /// Tighten every proven upper bound to an exact distance. This closes the
    /// winning-strategy subgraph: all defenses against a proved loss get exact
    /// values, even though unrelated alternatives can remain unknown.
    pub fn refine_wins(&mut self) -> Result<(), String> {
        loop {
            let mut open: Vec<_> = self
                .proof
                .bounds
                .iter()
                .filter_map(|(&key, b)| (b.within < 255 && b.not_within < b.within).then_some(key))
                .collect();
            open.sort_unstable();
            if open.is_empty() {
                return Ok(());
            }
            for key in open {
                loop {
                    let b = self.proof.bounds[&key];
                    if b.not_within >= b.within {
                        break;
                    }
                    let h = b.not_within + (b.within - b.not_within - 1) / 2;
                    self.query(Position(key & POSITION_MASK), ((key >> 52) & 1) as u8, h)?;
                }
            }
        }
    }
    pub fn exact(
        &mut self,
        state: &GameState,
        max_horizon: u8,
    ) -> Result<Option<Evaluation>, String> {
        if max_horizon == 255 {
            return Err("Maximum supported horizon is 254".into());
        }
        if let Some(value) = Evaluation::terminal(state) {
            return Ok(Some(value));
        }
        let pos = self.proof.space.encode(state);
        if (0..2).all(|target| self.proof.prevents_win(pos, target)) {
            return Ok(Some(Evaluation::Draw));
        }
        for h in 0..=max_horizon {
            for target in [pos.turn(), 1 - pos.turn()] {
                if self.query(pos, target, h)? {
                    return Ok(Some(Evaluation::Win {
                        winner: target,
                        plies: u32::from(h),
                    }));
                }
            }
        }
        Ok(None)
    }
}

impl Proof {
    /// Check certificates using the ordinary core model, not packed move generation.
    /// False bounds are monotone in the horizon; all witness queries decrease the
    /// requested horizon, even when a stronger cached bound is used as evidence.
    pub fn verify(&self) -> Result<(), String> {
        if self.magic != *b"RCGPRF02" || self.rules != crate::core::game::RULES_VERSION {
            return Err("Incompatible proof".into());
        }
        if self.space.totals.iter().any(|n| *n > 24) || self.space.colors[0] == self.space.colors[1]
        {
            return Err("Invalid proof space".into());
        }
        for (&key, &bounds) in &self.bounds {
            if bounds.not_within > bounds.within {
                return Err("Invalid bounds".into());
            }
            let state = self.validate_key(key)?;
            let target = ((key >> 52) & 1) as u8;
            let children: Vec<_> = state
                .legal_moves()
                .into_iter()
                .map(|m| {
                    let mut child = state;
                    child.apply_move(m).unwrap();
                    child
                })
                .collect();
            for (h, claim) in [
                (bounds.within, true),
                (bounds.not_within.wrapping_sub(1), false),
            ] {
                if h == 255 {
                    continue;
                }
                if let Some(base) = self.core_base(&state, target, h) {
                    if base != claim {
                        return Err("Incorrect base-case claim".into());
                    }
                    continue;
                }
                let child_claim = |s: &GameState| -> Option<bool> {
                    self.core_base(s, target, h - 1).or_else(|| {
                        let k = self.space.encode_board(s).0 | (u64::from(target) << 52);
                        self.bounds.get(&k).and_then(|b| b.query(h - 1))
                    })
                };
                let valid = if state.player_to_move.id == target {
                    if claim {
                        children.iter().any(|s| child_claim(s) == Some(true))
                    } else {
                        children.iter().all(|s| child_claim(s) == Some(false))
                    }
                } else if claim {
                    children.iter().all(|s| child_claim(s) == Some(true))
                } else {
                    children.iter().any(|s| child_claim(s) == Some(false))
                };
                if !valid {
                    return Err(format!(
                        "Missing minimax witness for {key}, h={h}, claim={claim}"
                    ));
                }
            }
        }
        // Safety is coinductive: terminal target wins are excluded, every target
        // move stays safe, and the defender has at least one safe move. Cycles are
        // allowed here because the objective is to avoid winning forever.
        for &key in &self.safe {
            let state = self.validate_key(key)?;
            let target = ((key >> 52) & 1) as u8;
            if let Some(base) = self.core_safe_base(&state, target) {
                if !base {
                    return Err("Safety certificate contains a target victory".into());
                }
                continue;
            }
            let children: Vec<_> = state
                .legal_moves()
                .into_iter()
                .map(|m| {
                    let mut child = state;
                    child.apply_move(m).unwrap();
                    self.core_safe_base(&child, target).unwrap_or_else(|| {
                        self.safe.contains(
                            &(self.space.encode_board(&child).0 | (u64::from(target) << 52)),
                        )
                    })
                })
                .collect();
            let valid = !children.is_empty()
                && if state.player_to_move.id == target {
                    children.iter().all(|b| *b)
                } else {
                    children.iter().any(|b| *b)
                };
            if !valid {
                return Err(format!("Open safety strategy at {key}"));
            }
        }
        Ok(())
    }
    fn validate_key(&self, key: u64) -> Result<GameState, String> {
        if key >> 53 != 0 {
            return Err("Invalid proof key".into());
        }
        let pos = Position(key & POSITION_MASK);
        if pos.board(0) & pos.board(1) != 0
            || (0..2).any(|p| pos.board(p).count_ones() > self.space.totals[p as usize] as u32)
        {
            return Err("Invalid packed board".into());
        }
        let state = self.space.decode(pos);
        state.validate().map_err(str::to_owned)?;
        if self.space.encode(&state) != pos {
            return Err("Noncanonical proof state".into());
        }
        Ok(state)
    }
    fn core_safe_base(&self, state: &GameState, target: u8) -> Option<bool> {
        if let Some(value) = Evaluation::terminal(state) {
            return Some(value.winner() != Some(target));
        }
        if self.space.totals[target as usize] < 3 {
            return Some(true);
        }
        let other_key = self.space.encode_board(state).0 | (u64::from(1 - target) << 52);
        // A proved opponent victory also prevents a target victory. Only the
        // positive horizon proof is used; positive proofs never depend on safety.
        self.bounds
            .get(&other_key)
            .filter(|b| b.within < 255)
            .map(|_| true)
    }
    fn packed_safe_base(&self, pos: Position, target: u8) -> Option<bool> {
        if let Some(winner) = pos.terminal() {
            return Some(winner != target as i8);
        }
        if self.space.totals[target as usize] < 3 {
            return Some(true);
        }
        let other_key = pos.0 | (u64::from(1 - target) << 52);
        self.bounds
            .get(&other_key)
            .filter(|b| b.within < 255)
            .map(|_| true)
    }
    fn prevents_win(&self, pos: Position, target: u8) -> bool {
        self.packed_safe_base(pos, target)
            .unwrap_or_else(|| self.safe.contains(&(pos.0 | (u64::from(target) << 52))))
    }
    /// Keep the greatest closed safety strategies inside the explored positions.
    /// Missing successors start unsafe; no horizon threshold implies safety.
    pub fn close_safety(&mut self) -> usize {
        let mut keys: Vec<_> = self
            .bounds
            .iter()
            .filter_map(|(&k, b)| (b.within == 255).then_some(k))
            .chain(self.safe.iter().copied())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        let ids: HashMap<_, _> = keys.iter().enumerate().map(|(i, &k)| (k, i)).collect();
        let mut parents = vec![Vec::<usize>::new(); keys.len()];
        let mut all = vec![false; keys.len()];
        let mut remaining = vec![0usize; keys.len()];
        let mut alive = vec![true; keys.len()];
        let mut queue = VecDeque::new();
        for (i, &key) in keys.iter().enumerate() {
            let pos = Position(key & POSITION_MASK);
            let target = ((key >> 52) & 1) as u8;
            if let Some(base) = self.packed_safe_base(pos, target) {
                if !base {
                    alive[i] = false;
                    queue.push_back(i);
                }
                continue;
            }
            all[i] = pos.turn() == target;
            let children = self.space.children(pos);
            let mut missing = false;
            for &child in &children.items[..children.len] {
                match self.packed_safe_base(child, target) {
                    Some(true) => remaining[i] += 1,
                    Some(false) => missing = true,
                    None => {
                        let child_key = child.0 | (u64::from(target) << 52);
                        if let Some(&j) = ids.get(&child_key) {
                            parents[j].push(i);
                            remaining[i] += 1;
                        } else {
                            missing = true;
                        }
                    }
                }
            }
            if remaining[i] == 0 || (all[i] && missing) {
                alive[i] = false;
                queue.push_back(i);
            }
        }
        while let Some(child) = queue.pop_front() {
            for &parent in &parents[child] {
                if !alive[parent] {
                    continue;
                }
                remaining[parent] -= 1;
                if all[parent] || remaining[parent] == 0 {
                    alive[parent] = false;
                    queue.push_back(parent);
                }
            }
        }
        self.safe = keys
            .into_iter()
            .enumerate()
            .filter_map(|(i, k)| alive[i].then_some(k))
            .collect();
        self.safe.len()
    }
    fn core_base(&self, state: &GameState, target: u8, horizon: u8) -> Option<bool> {
        if let Some(value) = Evaluation::terminal(state) {
            return Some(value.winner() == Some(target));
        }
        if !self.safe.is_empty()
            && self
                .safe
                .contains(&(self.space.encode_board(state).0 | (u64::from(target) << 52)))
        {
            return Some(false);
        }
        let count = state
            .cage
            .grid
            .iter()
            .flatten()
            .flatten()
            .filter(|c| **c == Some(state.players[target as usize].color))
            .count();
        if count + usize::from(state.remaining_cubies[target as usize]) < 3 {
            return Some(false);
        }
        let missing = 3usize.saturating_sub(count);
        let earliest = if missing == 0 {
            1
        } else {
            2 * missing - usize::from(state.player_to_move.id == target)
        };
        (usize::from(horizon) < earliest).then_some(false)
    }
    pub fn exact_values(&self) -> EvaluationMap {
        let mut values: EvaluationMap = self
            .bounds
            .iter()
            .filter_map(|(&key, b)| {
                if b.within == 255 || b.not_within != b.within {
                    return None;
                }
                let state = self.space.decode(Position(key & POSITION_MASK));
                Some((
                    state.position_key(),
                    Evaluation::Win {
                        winner: ((key >> 52) & 1) as u8,
                        plies: u32::from(b.within),
                    },
                ))
            })
            .collect();
        for &key in &self.safe {
            let pos = Position(key & POSITION_MASK);
            if (0..2).all(|target| self.prevents_win(pos, target)) {
                values.insert(self.space.decode(pos).position_key(), Evaluation::Draw);
            }
        }
        values
    }
    pub fn save(&self, path: &str) -> Result<(), String> {
        // Struct fields and this tuple have the same bincode representation.
        // Sort map entries so repeated precomputations produce identical bytes.
        let ordered: std::collections::BTreeMap<_, _> =
            self.bounds.iter().map(|(&k, &v)| (k, v)).collect();
        let bytes = bincode::encode_to_vec(
            (self.magic, self.rules, self.space, ordered, &self.safe),
            bincode::config::standard(),
        )
        .map_err(|e| e.to_string())?;
        let tmp = format!("{path}.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let config = bincode::config::standard().with_limit::<536870912>();
        let (proof, used): (Self, usize) = if bytes.starts_with(b"RCGPRF01") {
            // Version 1 had horizon bounds only. Its claims remain valid.
            type Legacy = ([u8; 8], u32, Space, HashMap<u64, Bounds>);
            let ((_, rules, space, bounds), used): (Legacy, usize) =
                bincode::decode_from_slice(&bytes, config).map_err(|e| e.to_string())?;
            (
                Self {
                    magic: *b"RCGPRF02",
                    rules,
                    space,
                    bounds,
                    safe: BTreeSet::new(),
                },
                used,
            )
        } else {
            bincode::decode_from_slice(&bytes, config).map_err(|e| e.to_string())?
        };
        if used != bytes.len() {
            return Err("Trailing proof data".into());
        }
        proof.verify()?;
        Ok(proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::retrograde;
    #[test]
    fn horizons_and_certificates_agree_with_complete_graphs() {
        for (a, b) in [(1, 1), (3, 0), (0, 3), (3, 1), (3, 2)] {
            let root = GameState::new(a, b);
            let graph =
                retrograde::solve(&root, &EvaluationMap::new(), Default::default()).unwrap();
            let mut search = Search::new(&root, Budget::default()).unwrap();
            let mut entries: Vec<_> = graph.values.iter().collect();
            entries.sort_by_key(|(k, _)| **k);
            for (&key, &value) in entries.into_iter().step_by(23) {
                let state = key.to_state();
                let pos = search.proof.space.encode(&state);
                for target in 0..2 {
                    for h in 0..=12 {
                        let expected =
                            value.winner() == Some(target) && value.plies().unwrap() <= h as u32;
                        assert_eq!(search.query(pos, target, h).unwrap(), expected);
                    }
                }
            }
            search.proof.close_safety();
            search.proof.verify().unwrap();
            for (k, v) in search.proof.exact_values() {
                assert_eq!(graph.values[&k], v);
            }
        }
    }
    #[test]
    fn full_inventory_opening_and_every_defense_have_exact_distances() {
        let root = GameState::new(12, 12);
        let mut search = Search::new(&root, Budget::default()).unwrap();
        assert_eq!(
            search.exact(&root, 11).unwrap(),
            Some(Evaluation::Win {
                winner: 0,
                plies: 11
            })
        );
        search.refine_wins().unwrap();
        search.proof.verify().unwrap();
        let values = search.proof.exact_values();
        let mut pending = vec![root];
        let mut seen = std::collections::HashSet::new();
        while let Some(state) = pending.pop() {
            if Evaluation::terminal(&state).is_some() || !seen.insert(state.position_key()) {
                continue;
            }
            let value = values[&state.position_key()];
            let Evaluation::Win { winner, plies } = value else {
                panic!("Unexpected draw");
            };
            let mut children = Vec::new();
            for m in state.legal_moves() {
                let mut child = state;
                child.apply_move(m).unwrap();
                let next = Evaluation::terminal(&child)
                    .or_else(|| values.get(&child.position_key()).copied());
                if state.player_to_move.id != winner {
                    assert!(
                        matches!(next, Some(Evaluation::Win { winner: w, plies: d }) if w == winner && d < plies)
                    );
                    children.push(child);
                } else if next
                    == Some(Evaluation::Win {
                        winner,
                        plies: plies - 1,
                    })
                {
                    children.push(child);
                }
            }
            assert!(!children.is_empty());
            if state.player_to_move.id != winner {
                assert_eq!(
                    children
                        .iter()
                        .map(|s| Evaluation::terminal(s)
                            .or_else(|| values.get(&s.position_key()).copied())
                            .unwrap()
                            .plies()
                            .unwrap())
                        .max(),
                    Some(plies - 1)
                );
            }
            pending.extend(children);
        }
        assert!(seen.len() > 10);
        let key = search.proof.space.encode(&root).0;
        search.proof.bounds.get_mut(&key).unwrap().within = 10;
        assert!(
            search.proof.verify().is_err(),
            "Corrupted opening proof must be rejected"
        );
    }

    #[test]
    fn safety_closure_matches_complete_graph_draws() {
        for (a, b) in [(3, 0), (3, 1), (3, 2)] {
            let root = GameState::new(a, b);
            let graph =
                retrograde::solve(&root, &EvaluationMap::new(), Default::default()).unwrap();
            let mut search = Search::new(&root, Budget::default()).unwrap();
            // Seed only the trivial horizon-zero failure for every nonterminal.
            // All draw conclusions must come from closure, not from search depth
            // or from the reference solver's outcome classifications.
            for key in graph.values.keys() {
                let state = key.to_state();
                if Evaluation::terminal(&state).is_some() {
                    continue;
                }
                let pos = search.proof.space.encode(&state);
                for target in 0..2 {
                    search.proof.bounds.insert(
                        pos.0 | (target << 52),
                        Bounds {
                            not_within: 1,
                            within: 255,
                        },
                    );
                }
            }
            search.proof.close_safety();
            search.proof.verify().unwrap();
            let values = search.proof.exact_values();
            for (k, v) in &graph.values {
                if Evaluation::terminal(&k.to_state()).is_none() {
                    assert_eq!(
                        values.get(k) == Some(&Evaluation::Draw),
                        *v == Evaluation::Draw
                    );
                }
            }
            let pos = search.proof.space.encode(&root);
            let safe_before = search.proof.safe.clone();
            search.proof.close_safety();
            assert_eq!(
                safe_before, search.proof.safe,
                "Safety closure is a fixed point"
            );
            if graph.values[&root.position_key()] == Evaluation::Draw {
                assert_eq!(search.exact(&root, 0).unwrap(), Some(Evaluation::Draw));
                assert!(!search.query(pos, 0, 254).unwrap());
            }
        }
    }

    #[test]
    fn missing_successors_and_forged_safety_are_not_draw_proofs() {
        let root = GameState::new(3, 3);
        let mut search = Search::new(&root, Budget::default()).unwrap();
        let pos = search.proof.space.encode(&root);
        for target in 0..2 {
            search.proof.bounds.insert(
                pos.0 | (target << 52),
                Bounds {
                    not_within: 1,
                    within: 255,
                },
            );
        }
        search.proof.verify().unwrap();
        assert_eq!(search.proof.close_safety(), 0);
        assert!(
            !search
                .proof
                .exact_values()
                .contains_key(&root.position_key())
        );
        search.proof.safe.extend([pos.0, pos.0 | (1 << 52)]);
        assert!(search.proof.verify().is_err());
    }

    #[test]
    fn incomplete_search_never_becomes_a_draw() {
        let root = GameState::new(3, 2);
        let mut search = Search::new(
            &root,
            Budget {
                max_positions: 2,
                max_calls: 1000,
            },
        )
        .unwrap();
        assert!(search.exact(&root, 12).is_err());
        search.proof.verify().unwrap();
        assert!(
            search
                .proof
                .exact_values()
                .values()
                .all(|v| *v != Evaluation::Draw)
        );
    }
}
