use crate::{
    app::{
        game_control::GameControl,
        hovered_move::use_hovered_move,
        utils::{slot_to_css, use_apply_move_callback},
    },
    core::{
        cubie::Cubie,
        game::{GameState, Outcome},
        r#move::{Layer, Move, Rotation},
    },
};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CageProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
    pub selected_colors: UseStateHandle<[Option<Cubie>; 2]>,
}

#[function_component(Cage)]
pub fn cage(props: &CageProps) -> Html {
    let player_to_move_color = crate::app::utils::selected_color(
        &props.game_state,
        props.game_state.player_to_move.id,
        props.selected_colors[props.game_state.player_to_move.id as usize],
    );
    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let (hovered_move, set_hovered_move) = use_hovered_move();
    let is_hovered_flip = hovered_move
        .0
        .as_ref()
        .map_or(false, |h| h.as_ref() == &Move::Flip);

    let won = props.game_state.won();
    let game_frozen = props.game_state.outcome().is_some();
    let apply_move = use_apply_move_callback(
        game_state_handle.clone(),
        history_handle.clone(),
        game_frozen,
    );

    let highlight_color = slot_to_css(player_to_move_color);
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
                                let drop_move = player_to_move_color.map(|color| Move::Drop { color, column: (i / 3, i % 3) });
                                let can_drop = z == 0 && i != 4 && cubie.is_none() && !game_frozen && drop_move.is_some();
                                let onclick = if can_drop {
                                    Some(apply_move.reform(move |_| drop_move.unwrap()))
                                } else {
                                    None
                                };

                                let hovered_drop_color = hovered_move.0.as_ref().and_then(|h| match h.as_ref() {
                                    Move::Drop { color, column } if *column == (i / 3, i % 3) && props.game_state.owner(*color) == Some(props.game_state.player_to_move.id) && props.game_state.remaining(*color) > 0 => Some(*color),
                                    _ => None,
                                });
                                let is_hovered_drop = hovered_drop_color.is_some() && !game_frozen;
                                let highlight_color = hovered_drop_color.map(|c| slot_to_css(Some(c))).unwrap_or(highlight_color);

                                let mut slot_classes = vec!["slot".to_string()];
                                if i == 4 { slot_classes.push("center-slot".to_string()); }
                                if is_hovered_drop && z == 0 { slot_classes.push("highlighted".to_string()); }
                                if let Some((_, _, line)) = won {
                                    let slot = [i / 3, i % 3, 2 - z];
                                    if line.iter().any(|s| s == &slot) {
                                        slot_classes.push("winning-line".to_string());
                                    }
                                }

                                html! {
                                    <div
                                        class={classes!(slot_classes)}
                                        role={if can_drop { "button" } else { "img" }}
                                        tabindex={if can_drop { "0" } else { "-1" }}
                                        aria-label={if can_drop { format!("Drop {} at {},{}", player_to_move_color.unwrap(), i / 3, i % 3) } else { cubie.map(|c| c.to_string()).unwrap_or_else(|| if i == 4 { "Blocked".into() } else { "Empty".into() }) }}
                                        onkeydown={{ let apply_move = apply_move.clone(); Callback::from(move |e: KeyboardEvent| { if can_drop && (e.key() == "Enter" || e.key() == " ") { e.prevent_default(); apply_move.emit(drop_move.unwrap()); } }) }}
                                        style={format!("--slot-color: {color}; --highlight-color: {highlight_color}; --slot-opacity: {slot_opacity};")}
                                        onclick={onclick}
                                        onmouseenter={
                                            if can_drop {
                                                let set_hovered_move = set_hovered_move.clone();
                                                let drop_move = Rc::new(drop_move.unwrap());
                                                Some(move |_| set_hovered_move.emit(Some(drop_move.clone())))
                                            } else {
                                                None
                                            }
                                        }
                                        onmouseleave={
                                            if can_drop {
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
                if let Some((winner, color, _)) = won {
                    html! { <h2 style="text-align: center;">{ if props.game_state.single_colors().is_some() { format!("{color} won!") } else { format!("Player {} wins with {color}!", winner.id + 1) } }</h2> }
                } else if props.game_state.outcome() == Some(Outcome::Draw) {
                    html! { <h2>{ "Draw: both players have a line." }</h2> }
                } else {
                    html! {}
                }
            }

            <GameControl game_state={props.game_state.clone()} history={props.history.clone()} />

        </div>
    }
}
