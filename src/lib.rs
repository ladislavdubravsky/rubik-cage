pub mod app;
mod compat;
pub mod core;
pub mod search;

use crate::{
    app::utils::{self, RELOAD_FLAG_KEY, STORAGE_KEY},
    core::{game::GameState, snapshot},
};
use app::{
    agent::EvaluationTask, cage::Cage, evaluation::EvaluationProvider,
    hovered_move::HoveredMoveProvider, player::PlayerPanel,
};
use web_sys::window;
use yew::prelude::*;
use yew_agent::oneshot::OneshotProvider;

#[function_component(App)]
pub fn app() -> Html {
    let game_state = use_state(|| {
        if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
            if let Ok(Some(flag)) = storage.get_item(RELOAD_FLAG_KEY) {
                if flag == "true" {
                    storage.remove_item(RELOAD_FLAG_KEY).ok();
                    if let Ok(Some(hex)) = storage.get_item(STORAGE_KEY) {
                        if let Some(bytes) = utils::hex_to_bytes(&hex) {
                            if let Ok(state) = snapshot::decode(&bytes) {
                                return state;
                            }
                        }
                    }
                }
            }
        }
        GameState::new(12, 12)
    });
    let history = use_state(|| Vec::new());
    let selected_colors = use_state(|| [None::<crate::core::cubie::Cubie>; 2]);

    // Save game state to LocalStorage on any change
    {
        let game_state = game_state.clone();
        use_effect_with(game_state.clone(), move |gs| {
            let bin = snapshot::encode(gs).unwrap();
            let hex = utils::bytes_to_hex(&bin);
            if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                storage.set_item(STORAGE_KEY, &hex).ok();
            }
            || ()
        });
    }

    html! {
        <div class="app">
            <h1>{ "Rubik's Cage Simulator" }</h1>
            <p>
                { "Place cubies, rotate layers, and try to get three in a line! " }
                <a href="https://github.com/ladislavdubravsky/rubik-cage" target="_blank" rel="noopener noreferrer">
                    { "Read more & source code" }
                </a>
                { " 🦀" }
            </p>
            <OneshotProvider<EvaluationTask> path="/rubik-cage/worker.js">
                <EvaluationProvider state={game_state.clone()}>
                <HoveredMoveProvider state={*game_state}>
                    <div class="game-area">
                        <PlayerPanel
                            game_state={game_state.clone()}
                            player={game_state.players[0]}
                            selected_colors={selected_colors.clone()}
                            history={history.clone()}
                        />
                        <Cage game_state={game_state.clone()} history={history.clone()} selected_colors={selected_colors.clone()} />
                        <PlayerPanel
                            game_state={game_state.clone()}
                            player={game_state.players[1]}
                            selected_colors={selected_colors.clone()}
                            history={history.clone()}
                        />
                    </div>
                </HoveredMoveProvider>
                </EvaluationProvider>
            </OneshotProvider<EvaluationTask>>
        </div>
    }
}
