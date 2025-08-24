use crate::core::game::GameState;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct GameControlProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
}

#[function_component(GameControl)]
pub fn game_control(props: &GameControlProps) -> Html {
    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let undo = Callback::from(move |_| {
        let mut new_history = (*history_handle).clone();
        if let Some(prev_state) = new_history.pop() {
            game_state_handle.set(prev_state);
            history_handle.set(new_history);
        }
    });

    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let restart = Callback::from(move |_| {
        game_state_handle.set(GameState::new(12, 12));
        history_handle.set(Vec::new());
    });

    let export = Callback::from(|_| {
        // TODO: Implement export position to file
        web_sys::window()
            .unwrap()
            .alert_with_message("Export position not implemented")
            .ok();
    });

    let import = Callback::from(|_| {
        // TODO: Implement import position from file
        web_sys::window()
            .unwrap()
            .alert_with_message("Import position not implemented")
            .ok();
    });

    html! {
        <div class="game-control">
            <button class="control-button" onclick={undo} disabled={(*props.history).is_empty()}>{ "Undo last move" }</button>
            <button class="control-button" onclick={restart}>{ "Restart the game" }</button>
            <button class="control-button" onclick={export}>{ "Export position" }</button>
            <button class="control-button" onclick={import}>{ "Import position" }</button>
        </div>
    }
}
