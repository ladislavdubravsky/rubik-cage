use crate::{
    core::{game::GameState, r#move::Move},
    search::naive::Evaluation,
};
use std::{cell::RefCell, collections::HashMap};
use yew::prelude::*;

pub fn apply_move_callback(
    game_state_handle: UseStateHandle<GameState>,
    history_handle: UseStateHandle<Vec<GameState>>,
    game_frozen: bool,
) -> Callback<Move> {
    let game_state_handle = game_state_handle.clone();
    let history_handle = history_handle.clone();
    Callback::from(move |m: Move| {
        if !game_frozen {
            let mut new_state = (*game_state_handle).clone();
            if new_state.apply_move(m).is_ok() {
                let mut new_history = (*history_handle).clone();
                new_history.push((*game_state_handle).clone());
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
    eval: &RefCell<HashMap<u64, Evaluation>>,
) -> Vec<Move> {
    let mut moves_with_eval: Vec<(Move, Option<Evaluation>)> = moves
        .into_iter()
        .map(|mv| {
            let mut new_state = game_state.clone();
            new_state.apply_move_normalize(mv.clone()).unwrap();
            let eval_map = eval.borrow();
            let eval = eval_map.get(&new_state.zobrist_hash).cloned();
            (mv, eval)
        })
        .collect();
    let player_id = game_state.player_to_move.id;
    moves_with_eval.sort_by(|a, b| {
        let eval_a = &a.1;
        let eval_b = &b.1;
        fn class(eval: &Option<Evaluation>, player_id: u8) -> u8 {
            match eval {
                Some(Evaluation { score: 1, .. }) if player_id == 0 => 0,
                Some(Evaluation { score: -1, .. }) if player_id == 1 => 0,
                Some(Evaluation { score: 0, .. }) => 1,
                Some(Evaluation { score: -1, .. }) if player_id == 0 => 2,
                Some(Evaluation { score: 1, .. }) if player_id == 1 => 2,
                _ => 3,
            }
        }
        let class_a = class(eval_a, player_id);
        let class_b = class(eval_b, player_id);
        match class_a.cmp(&class_b) {
            std::cmp::Ordering::Equal => {
                match (eval_a, eval_b) {
                    // Wins: shortest moves_to_wl first
                    (
                        Some(Evaluation {
                            score: 1,
                            moves_to_wl: ma,
                        }),
                        Some(Evaluation {
                            score: 1,
                            moves_to_wl: mb,
                        }),
                    ) if player_id == 0 => ma.cmp(mb),
                    (
                        Some(Evaluation {
                            score: -1,
                            moves_to_wl: ma,
                        }),
                        Some(Evaluation {
                            score: -1,
                            moves_to_wl: mb,
                        }),
                    ) if player_id == 1 => ma.cmp(mb),
                    // Losses: longest moves_to_wl first
                    (
                        Some(Evaluation {
                            score: -1,
                            moves_to_wl: ma,
                        }),
                        Some(Evaluation {
                            score: -1,
                            moves_to_wl: mb,
                        }),
                    ) if player_id == 0 => mb.cmp(ma),
                    (
                        Some(Evaluation {
                            score: 1,
                            moves_to_wl: ma,
                        }),
                        Some(Evaluation {
                            score: 1,
                            moves_to_wl: mb,
                        }),
                    ) if player_id == 1 => mb.cmp(ma),
                    // Draws or unknown: keep original order
                    _ => std::cmp::Ordering::Equal,
                }
            }
            ord => ord,
        }
    });
    moves_with_eval.into_iter().map(|(mv, _)| mv).collect()
}
