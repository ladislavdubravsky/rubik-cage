pub mod app;
pub mod core;
pub mod search;

use crate::{core::game::GameState, search::naive::Evaluation};
use app::{
    agent::EvaluationTask, cage::Cage, hovered_move::HoveredMoveProvider, player::PlayerPanel,
};
use std::collections::HashMap;
use yew::prelude::*;
use yew_agent::oneshot::OneshotProvider;

#[function_component(App)]
pub fn app() -> Html {
    let game_state = use_state(|| GameState::new(12, 12));
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

    html! {
        <div class="app">
            <h1>{ "Rubik's Cage Simulator" }</h1>
            <p>
                { "Place cubies, rotate layers, and try to get three in a line! " }
                <a href="https://github.com/ladislavdubravsky/rubik-cage" target="_blank" rel="noopener noreferrer">
                    { "Read more & source code" }
                </a>
                { "🦀" }
            </p>
            <OneshotProvider<EvaluationTask> path="/worker.js">
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
