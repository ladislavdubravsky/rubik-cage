mod app;
mod core;
mod search;

use crate::core::{game::GameState, r#move::Move};
use app::{cage::Cage, player::PlayerPanel};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    let game_state = use_state(|| GameState::new(12, 12));

    // TODO: this is just temporary 2 cubie drop before we get interactive drops
    let initialized = use_state(|| false);
    {
        let game_state = game_state.clone();
        let initialized = initialized.clone();
        use_effect(move || {
            if !*initialized {
                let mut new_state = (*game_state).clone();
                new_state
                    .apply_move(Move::Drop {
                        color: new_state.players[0].color,
                        column: (0, 0),
                    })
                    .unwrap();
                new_state
                    .apply_move(Move::Drop {
                        color: new_state.players[1].color,
                        column: (0, 0),
                    })
                    .unwrap();
                game_state.set(new_state);
                initialized.set(true);
            }
            || ()
        });
    }

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
