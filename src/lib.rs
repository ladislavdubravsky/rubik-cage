pub mod app;
pub mod core;
pub mod search;

use crate::{
    app::utils::{self, RELOAD_FLAG_KEY, STORAGE_KEY},
    core::game::GameState,
    search::naive::Evaluation,
};
use app::{
    agent::EvaluationTask, cage::Cage, hovered_move::HoveredMoveProvider, player::PlayerPanel,
};
use std::collections::HashMap;
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
                            if let Ok((state, _)) = bincode::decode_from_slice::<GameState, _>(
                                &bytes,
                                bincode::config::standard(),
                            ) {
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

    // Load precomputed evaluations for hardest-to-compute positions.
    // Evaluations for further positions will be calculated on the fly when needed.
    let eval = use_mut_ref(|| {
        const EVAL_BIN: &[u8] = include_bytes!("../assets/eval.bin");
        let config = bincode::config::standard();
        let (map, _len): (HashMap<u64, Evaluation>, usize) =
            bincode::decode_from_slice(EVAL_BIN, config).unwrap();
        map
    });

    // Save game state to LocalStorage on any change
    {
        let game_state = game_state.clone();
        use_effect_with(game_state.clone(), move |gs| {
            let bin = bincode::encode_to_vec(&**gs, bincode::config::standard()).unwrap();
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
                <HoveredMoveProvider>
                    <div class="game-area">
                        <PlayerPanel
                            game_state={game_state.clone()}
                            player={game_state.players[0]}
                            history={history.clone()}
                            eval={eval.clone()}
                        />
                        <Cage game_state={game_state.clone()} history={history.clone()} />
                        <PlayerPanel
                            game_state={game_state.clone()}
                            player={game_state.players[1]}
                            history={history.clone()}
                            eval={eval.clone()}
                        />
                    </div>
                </HoveredMoveProvider>
            </OneshotProvider<EvaluationTask>>
        </div>
    }
}
