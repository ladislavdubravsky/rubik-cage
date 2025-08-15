mod app;
mod core;
mod search;

use crate::core::game::GameState;
use app::{cage::Cage, player::PlayerPanel};
use yew::prelude::*;

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
