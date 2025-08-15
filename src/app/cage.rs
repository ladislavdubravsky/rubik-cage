use crate::{app::hovered_move::use_hovered_move, core::{
    game::GameState,
    r#move::{Layer, Move, Rotation},
}};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CageProps {
    pub game_state: UseStateHandle<GameState>,
}

#[function_component(Cage)]
pub fn cage(props: &CageProps) -> Html {
    let player_to_move_color = props.game_state.player_to_move.color;

    let game_state_handle = props.game_state.clone();
    let apply_move = Callback::from(move |m: Move| {
        let mut new_state = (*game_state_handle).clone();
        if new_state.apply_move(m).is_ok() {
            game_state_handle.set(new_state);
        }
    });

    let (hovered_move, set_hovered_move) = use_hovered_move();
    let is_hovered_flip = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &Move::Flip);

    html! {
        <div class="cage">
            { for [Layer::Up, Layer::Equator, Layer::Down].iter().enumerate().map(|(z, layer)| {
                let rotate_cw = Move::RotateLayer { layer: *layer, rotation: Rotation::Clockwise };
                let rotate_ccw = Move::RotateLayer { layer: *layer, rotation: Rotation::CounterClockwise };

                let is_hovered_cw = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &rotate_cw);
                let is_hovered_ccw = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &rotate_ccw);

                html! {
                    <div class="layer">
                        <button
                            class={classes!("rotate-button", if is_hovered_ccw { "highlighted" } else { "" })}
                            style={if is_hovered_ccw { format!("--highlight-color: {};", player_to_move_color) } else { String::new() }}
                            onclick={apply_move.reform(move |_| rotate_ccw)}
                            disabled={props.game_state.last_move == Some(rotate_cw)}
                            onmouseenter={{
                                let set_hovered_move = set_hovered_move.clone();
                                let rotate_ccw = Rc::new(rotate_ccw.clone());
                                move |_| set_hovered_move.emit(Some(rotate_ccw.clone()))
                            }}
                            onmouseleave={{
                                let set_hovered_move = set_hovered_move.clone();
                                move |_| set_hovered_move.emit(None)
                            }}
                        >{ "↻" }</button>

                        <div class="grid">
                            { for (0..9).map(|i| {
                                let cubie = props.game_state.cage.grid[i / 3][i % 3][2 - z];
                                let color = cubie.as_ref().map(|c| c.to_string()).unwrap_or("#444".into());

                                // Cubie drops are implemented by clicking on top layer slots.
                                let onclick = if z == 0 && i != 4 && cubie.is_none() {
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

                                let drop_move = Move::Drop {
                                    color: player_to_move_color.clone(),
                                    column: (i / 3, i % 3),
                                };
                                let is_hovered_drop = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &drop_move);

                                html! {
                                    <div
                                        class={classes!(
                                            "slot",
                                            if i == 4 { "center-slot" } else { "" },
                                            if is_hovered_drop && z == 0 { "highlighted" } else { "" }
                                        )}
                                        style={format!("--slot-color: {color}; --highlight-color: {player_to_move_color};")}
                                        onclick={onclick}
                                        onmouseenter={
                                            if z == 0 && i != 4 && cubie.is_none() {
                                                let set_hovered_move = set_hovered_move.clone();
                                                let drop_move = Rc::new(drop_move.clone());
                                                Some(move |_| set_hovered_move.emit(Some(drop_move.clone())))
                                            } else {
                                                None
                                            }
                                        }
                                        onmouseleave={
                                            if z == 0 && i != 4 && cubie.is_none() {
                                                let set_hovered_move = set_hovered_move.clone();
                                                Some(move |_| set_hovered_move.emit(None))
                                            } else {
                                                None
                                            }
                                        }
                                    />
                                }
                            }) }
                        </div>

                        <button
                            class={classes!("rotate-button", if is_hovered_cw { "highlighted" } else { "" })}
                            style={if is_hovered_cw { format!("--highlight-color: {};", player_to_move_color) } else { String::new() }}
                            onclick={apply_move.reform(move |_| rotate_cw)}
                            disabled={props.game_state.last_move == Some(rotate_ccw)}
                            onmouseenter={{
                                let set_hovered_move = set_hovered_move.clone();
                                let rotate_cw = Rc::new(rotate_cw.clone());
                                move |_| set_hovered_move.emit(Some(rotate_cw.clone()))
                            }}
                            onmouseleave={{
                                let set_hovered_move = set_hovered_move.clone();
                                move |_| set_hovered_move.emit(None)
                            }}
                        >{ "↺" }</button>
                    </div>
                }
            }) }

            <button
                class={if is_hovered_flip { "highlighted" } else { "" }}
                style={if is_hovered_flip { format!("--highlight-color: {};", player_to_move_color) } else { String::new() }}
                onclick={apply_move.reform(|_| Move::Flip)}
                disabled={props.game_state.last_move == Some(Move::Flip)}
                onmouseenter={{
                    let set_hovered_move = set_hovered_move.clone();
                    let flip = Rc::new(Move::Flip);
                    move |_| set_hovered_move.emit(Some(flip.clone()))
                }}
                onmouseleave={{
                    let set_hovered_move = set_hovered_move.clone();
                    move |_| set_hovered_move.emit(None)
                }}
            >{ "Flip" }</button>
        </div>
    }
}
