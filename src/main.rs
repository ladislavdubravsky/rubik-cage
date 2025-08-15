mod app;
mod core;
mod search;

use crate::core::game::GameState;
use app::{cage::Cage, player::PlayerPanel};
use std::{collections::HashMap, sync::LazyLock};
use yew::prelude::*;

pub static EVAL: LazyLock<HashMap<u64, isize>> = LazyLock::new(|| {
    const EVAL_BIN: &[u8] = include_bytes!("../assets/eval.bin");
    let config = bincode::config::standard();
    let (map, _len): (HashMap<u64, isize>, usize) =
        bincode::decode_from_slice(EVAL_BIN, config).unwrap();
    map
});

#[function_component(App)]
fn app() -> Html {
    let game_state = use_state(|| GameState::new(12, 12));

    html! {
        <div class="app">
            <h1>{ "Rubik's Cage Simulator" }</h1>
            <p>{ "Place cubies, rotate layers, and try to get three in a line!" }</p>
            <div class="game-area">
                <PlayerPanel game_state={game_state.clone()} player={game_state.players[0]} />
                <Cage game_state={game_state.clone()} />
                <PlayerPanel game_state={game_state.clone()} player={game_state.players[1]} />
            </div>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
