use crate::{
    app::{
        ai::{AiContext, AiInteraction},
        evaluation::EvaluationContext,
    },
    core::{cubie::Cubie, game::GameState, r#move::Move},
    search::Evaluation,
};
use yew::prelude::*;

pub const STORAGE_KEY: &str = "rubik_cage_position";
pub const RELOAD_FLAG_KEY: &str = "load_position_on_next_reload";

#[hook]
pub fn use_apply_move_callback(
    game_state_handle: UseStateHandle<GameState>,
    history_handle: UseStateHandle<Vec<GameState>>,
    game_frozen: bool,
) -> Callback<Move> {
    let ai = use_context::<AiContext>();
    let game_state_handle = game_state_handle.clone();
    let history_handle = history_handle.clone();
    Callback::from(move |m: Move| {
        if !game_frozen {
            let mut new_state = (*game_state_handle).clone();
            if new_state.apply_move(m).is_ok() {
                let mut new_history = (*history_handle).clone();
                new_history.push((*game_state_handle).clone());
                if let Some(ai) = &ai {
                    ai.0.emit(AiInteraction::Move {
                        state: new_state,
                        history: new_history.clone(),
                    });
                }
                history_handle.set(new_history);
                game_state_handle.set(new_state);
            }
        }
    })
}

/// Sort moves by evaluation: wins for player first (shortest moves_to_wl), then draws, then losses
/// (longest loss first), unknowns last.
pub fn sort_moves_by_evaluation(
    moves: Vec<Move>,
    game_state: &GameState,
    eval: &EvaluationContext,
) -> Vec<Move> {
    let mut scored: Vec<_> = moves
        .into_iter()
        .map(|m| {
            let mut child = *game_state;
            child.apply_move(m).unwrap();
            let priority = match eval.get(&child) {
                Some(Evaluation::Win { winner, plies })
                    if winner == game_state.player_to_move.id =>
                {
                    (0, plies)
                }
                Some(Evaluation::Draw) => (1, 0),
                Some(Evaluation::Win { plies, .. }) => (2, u32::MAX - plies),
                None => (3, 0),
            };
            (m, priority)
        })
        .collect();
    scored.sort_by_key(|(_, priority)| *priority);
    scored.into_iter().map(|(m, _)| m).collect()
}

pub fn slot_to_css(cubie: Option<Cubie>) -> &'static str {
    match cubie {
        Some(Cubie::Blue) => "var(--cubie-blue)",
        Some(Cubie::Red) => "var(--cubie-red)",
        Some(Cubie::White) => "var(--cubie-white)",
        Some(Cubie::Yellow) => "var(--cubie-yellow)",
        Some(Cubie::Orange) => "var(--cubie-orange)",
        Some(Cubie::Green) => "var(--cubie-green)",
        None => "var(--slot-empty)",
    }
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    Some(
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect::<Option<Vec<u8>>>()?,
    )
}

/// UI selection is not part of the game/search identity. Repair it on every render.
pub fn selected_color(state: &GameState, player: u8, preferred: Option<Cubie>) -> Option<Cubie> {
    preferred
        .filter(|&c| state.owner(c) == Some(player) && state.remaining(c) > 0)
        .or_else(|| state.colors_for(player).find(|&c| state.remaining(c) > 0))
}

pub fn move_label(state: &GameState, m: Move) -> String {
    match m {
        Move::Drop {
            color,
            column: (x, y),
        } if state.single_colors().is_none() => format!("Drop {color} at {x},{y}"),
        _ => m.to_string(),
    }
}
