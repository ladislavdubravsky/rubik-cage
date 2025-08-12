use crate::core::{game::GameState, r#move::Move};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CageProps {
    pub game_state: UseStateHandle<GameState>,
}

#[function_component(Cage)]
pub fn cage(props: &CageProps) -> Html {
    let player_to_move_color = props.game_state.player_to_move.color;

    let game_state_handle = props.game_state.clone();
    let on_flip = Callback::from(move |_| {
        let mut new_state = (*game_state_handle).clone();
        if new_state.apply_move(Move::Flip).is_ok() {
            game_state_handle.set(new_state);
        }
    });

    html! {
        <div class="cage">
            { for (0..3).rev().map(|z| html! {
                <div class="layer">
                    <div class="grid">
                        { for (0..9).map(|i| {
                            let cubie = props.game_state.cage.grid[i / 3][i % 3][z];
                            let color = cubie.as_ref().map(|c| c.to_string()).unwrap_or("#444".into());

                            // Cubie drop moves are implemented by clicking on top layer slots.
                            let onclick = if z == 2 && i != 4 && cubie.is_none() {
                                let game_state_handle = props.game_state.clone();
                                let color = player_to_move_color.clone();

                                Some(Callback::from(move |_| {
                                    let mut new_state = (*game_state_handle).clone();
                                    let res = new_state.apply_move(Move::Drop {
                                        color,
                                        column: (i / 3, i % 3),
                                    });
                                    if res.is_ok() {
                                        game_state_handle.set(new_state);
                                    }
                                }))
                            } else {
                                None
                            };

                            html! {
                                <div
                                    class={classes!(
                                        "slot",
                                        if z == 2 && i != 4 && cubie.is_none() { "top-slot" } else { "" },
                                        if i == 4 { "center-slot" } else { "" }
                                    )}
                                    style={format!("--slot-color: {color}; --slot-hover-color: {player_to_move_color};")}
                                    onclick={onclick}
                                />
                            }
                        }) }
                    </div>
                </div>
            }) }
            <button onclick={on_flip}>{ "Flip" }</button>
        </div>
    }
}
