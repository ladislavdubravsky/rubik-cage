//! Exact graph minimax. The historical `naive.rs` is deliberately not compiled.
pub mod bounded;
pub mod cache;
pub mod packed;
pub mod retrograde;

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
