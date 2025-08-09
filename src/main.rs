mod app;
mod core;
mod search;

use app::{cube::Cube, player::PlayerPanel};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <div class="app">
            <h1>{ "Rubik's Cage Simulator" }</h1>
            <p>{ "Place cubies, rotate layers, and try to get three in a line!" }</p>
            <div class="game-area">
                <PlayerPanel player_id={1} is_turn={true} />
                <Cube />
                <PlayerPanel player_id={2} is_turn={false} />
            </div>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
