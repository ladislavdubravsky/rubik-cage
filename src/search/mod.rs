//! Exact evaluation and separate approximate move search. Historical `naive.rs` is not compiled.
pub mod ai;
pub mod bounded;
pub mod cache;
pub mod general;
pub mod general_compact;
mod general_graph;
#[cfg(feature = "search-audit")]
pub mod general_reference;
pub mod packed;
pub mod retrograde;
mod single_color;

use crate::core::{
    game::{GameState, Outcome},
    position::PositionKey,
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type EvaluationMap = HashMap<PositionKey, Evaluation>;

/// Exact, history-independent values. Unknown is represented by absence, never Draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum Evaluation {
    Win { winner: u8, plies: u32 },
    Draw,
}

impl Evaluation {
    pub fn swapped_players(self) -> Self {
        match self {
            Self::Win { winner, plies } => Self::Win {
                winner: 1 - winner,
                plies,
            },
            Self::Draw => Self::Draw,
        }
    }

    pub fn terminal(state: &GameState) -> Option<Self> {
        state.outcome().map(|outcome| match outcome {
            Outcome::Win(winner) => Self::Win { winner, plies: 0 },
            Outcome::Draw => Self::Draw,
        })
    }

    pub fn winner(self) -> Option<u8> {
        match self {
            Self::Win { winner, .. } => Some(winner),
            Self::Draw => None,
        }
    }

    pub fn plies(self) -> Option<u32> {
        match self {
            Self::Win { plies, .. } => Some(plies),
            Self::Draw => None,
        }
    }
}

/// Check conflicts before modifying anything; exact answers cannot overwrite each other.
pub fn merge_exact(target: &mut EvaluationMap, incoming: EvaluationMap) -> Result<(), String> {
    for (key, value) in &incoming {
        if let Some(previous) = target.get(key)
            && previous != value
        {
            return Err(format!(
                "Conflicting exact evaluations: {previous:?} vs {value:?}"
            ));
        }
    }
    target.extend(incoming);
    Ok(())
}

/// Reuse exact results under player relabeling, including unequal inventories.
/// The full key swaps reserves, board ownership and turn together.
pub fn lookup_exact(values: &EvaluationMap, state: &GameState) -> Option<Evaluation> {
    Evaluation::terminal(state).or_else(|| {
        let key = state.position_key();
        values.get(&key).copied().or_else(|| {
            values
                .get(&key.swapped_players())
                .copied()
                .map(Evaluation::swapped_players)
        })
    })
}

pub fn include_player_swaps(mut values: EvaluationMap) -> Result<EvaluationMap, String> {
    let swapped = values
        .iter()
        .map(|(&k, &v)| (k.swapped_players(), v.swapped_players()))
        .collect();
    merge_exact(&mut values, swapped)?;
    Ok(values)
}
