use crate::{
    app::{
        game_control::GameControl,
        hovered_move::use_hovered_move,
        utils::{apply_move_callback, slot_to_css},
    },
    core::{
        game::GameState,
        r#move::{Layer, Move, Rotation},
    },
};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CageProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
}

#[function_component(Cage)]
pub fn cage(props: &CageProps) -> Html {
    let player_to_move_color = props.game_state.player_to_move.color;
    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let (hovered_move, set_hovered_move) = use_hovered_move();
    let is_hovered_flip = hovered_move
        .0
        .as_ref()
        .map_or(false, |h| h.as_ref() == &Move::Flip);

    let won = props.game_state.won();
    let game_frozen = won.is_some();
    let apply_move = apply_move_callback(
        game_state_handle.clone(),
        history_handle.clone(),
        game_frozen,
    );

    let highlight_color = slot_to_css(Some(props.game_state.player_to_move.color));
    let slot_opacity = if game_frozen { "0.3" } else { "1.0" };
    let flip_disabled = game_frozen || props.game_state.last_move == Some(Move::Flip);

    html! {
        <div class="cage">
            { for [Layer::Up, Layer::Equator, Layer::Down].iter().enumerate().map(|(z, layer)| {
                let rotate_cw = Move::RotateLayer { layer: *layer, rotation: Rotation::Clockwise };
                let rotate_ccw = Move::RotateLayer { layer: *layer, rotation: Rotation::CounterClockwise };

                let is_hovered_cw = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &rotate_cw);
                let is_hovered_ccw = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &rotate_ccw);

                let cw_disabled = game_frozen || props.game_state.last_move == Some(rotate_ccw);
                let ccw_disabled = game_frozen || props.game_state.last_move == Some(rotate_cw);

                html! {
                    <div class="layer">
                        <button
                            class={classes!("control-button", if is_hovered_ccw && !ccw_disabled { "highlighted" } else { "" })}
                            style={if is_hovered_ccw { format!("--highlight-color: {};", highlight_color) } else { String::new() }}
                            onclick={apply_move.reform(move |_| rotate_ccw)}
                            disabled={ccw_disabled}
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
                                let color = slot_to_css(cubie);

                                // Cubie drops are implemented by clicking on top layer slots.
                                let drop_move = Move::Drop {
                                    color: player_to_move_color.clone(),
                                    column: (i / 3, i % 3),
                                };
                                let onclick = if z == 0 && i != 4 && cubie.is_none() && !game_frozen {
                                    Some(apply_move.reform(move |_| drop_move.clone()))
                                } else {
                                    None
                                };

                                let is_hovered_drop = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &drop_move);

                                let mut slot_classes = vec!["slot".to_string()];
                                if i == 4 { slot_classes.push("center-slot".to_string()); }
                                if is_hovered_drop && z == 0 { slot_classes.push("highlighted".to_string()); }
                                if let Some((_, line)) = won {
                                    let slot = [i / 3, i % 3, 2 - z];
                                    if line.iter().any(|s| s == &slot) {
                                        slot_classes.push("winning-line".to_string());
                                    }
                                }

                                html! {
                                    <div
                                        class={classes!(slot_classes)}
                                        style={format!("--slot-color: {color}; --highlight-color: {highlight_color}; --slot-opacity: {slot_opacity};")}
                                        onclick={onclick}
                                        onmouseenter={
                                            if z == 0 && i != 4 && cubie.is_none() && !game_frozen {
                                                let set_hovered_move = set_hovered_move.clone();
                                                let drop_move = Rc::new(drop_move.clone());
                                                Some(move |_| set_hovered_move.emit(Some(drop_move.clone())))
                                            } else {
                                                None
                                            }
                                        }
                                        onmouseleave={
                                            if z == 0 && i != 4 && cubie.is_none() && !game_frozen {
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
                            class={classes!("control-button", if is_hovered_cw && !cw_disabled { "highlighted" } else { "" })}
                            style={if is_hovered_cw { format!("--highlight-color: {};", highlight_color) } else { String::new() }}
                            onclick={apply_move.reform(move |_| rotate_cw)}
                            disabled={cw_disabled}
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
                class={classes!("control-button", if is_hovered_flip && !flip_disabled { "highlighted" } else { "" })}
                style={if is_hovered_flip { format!("--highlight-color: {};", highlight_color) } else { String::new() }}
                onclick={apply_move.reform(|_| Move::Flip)}
                disabled={flip_disabled}
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

            {
                if let Some((winner, _)) = won {
                    html! { <h2 style="text-align: center;">{ format!("{} won!", winner.color) }</h2> }
                } else {
                    html! {}
                }
            }

            <GameControl game_state={props.game_state.clone()} history={props.history.clone()} />

        </div>
    }
}
