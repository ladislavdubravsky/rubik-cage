//! Goal-directed finite-horizon minimax. A failed query is only a lower bound on
//! distance, never a draw. Combining adjacent failed/successful horizons yields
//! exact distances. Every published bound is independently checkable against core.
use super::{
    Evaluation, EvaluationMap,
    packed::{POSITION_MASK, Position, Space},
};
use crate::core::game::GameState;
use bincode::{Decode, Encode};
use std::collections::HashMap;

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
                magic: *b"RCGPRF01",
                rules: crate::core::game::RULES_VERSION,
                space: Space::new(state).map_err(str::to_owned)?,
                bounds: HashMap::new(),
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
        if self.magic != *b"RCGPRF01" || self.rules != crate::core::game::RULES_VERSION {
            return Err("Incompatible proof".into());
        }
        if self.space.totals.iter().any(|n| *n > 24) || self.space.colors[0] == self.space.colors[1]
        {
            return Err("Invalid proof space".into());
        }
        for (&key, &bounds) in &self.bounds {
            if key >> 53 != 0 || bounds.not_within > bounds.within {
                return Err("Invalid bounds".into());
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
        Ok(())
    }
    fn core_base(&self, state: &GameState, target: u8, horizon: u8) -> Option<bool> {
        if let Some(value) = Evaluation::terminal(state) {
            return Some(value.winner() == Some(target));
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
        self.bounds
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
            .collect()
    }
    pub fn save(&self, path: &str) -> Result<(), String> {
        // Struct fields and this tuple have the same bincode representation.
        // Sort map entries so repeated precomputations produce identical bytes.
        let ordered: std::collections::BTreeMap<_, _> =
            self.bounds.iter().map(|(&k, &v)| (k, v)).collect();
        let bytes = bincode::encode_to_vec(
            (self.magic, self.rules, self.space, ordered),
            bincode::config::standard(),
        )
        .map_err(|e| e.to_string())?;
        let tmp = format!("{path}.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let (proof, used): (Self, usize) = bincode::decode_from_slice(
            &bytes,
            bincode::config::standard().with_limit::<536870912>(),
        )
        .map_err(|e| e.to_string())?;
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
