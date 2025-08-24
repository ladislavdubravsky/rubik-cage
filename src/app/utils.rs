use crate::core::{game::GameState, r#move::Move};
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
