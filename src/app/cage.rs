use crate::{app::hovered_move::use_hovered_move, core::{
    game::GameState, r#move::{Layer, Move, Rotation}
}};
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
    let is_hovered_flip = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == &Move::Flip);

    let won = props.game_state.won();
    let game_frozen = won.is_some();

    let apply_move = {
        let game_state_handle = game_state_handle.clone();
        let history_handle = history_handle.clone();
        Callback::from(move |m: Move| {
            if !game_frozen {
                let mut new_state = (*game_state_handle).clone();
                if new_state.apply_move(m).is_ok() {
                    let mut new_history = (*history_handle).clone();
                    new_history.push((*game_state_handle).clone());
                    history_handle.set(new_history);
                    game_state_handle.set(new_state);
                }
            }
        })
    };

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
                            disabled={game_frozen || props.game_state.last_move == Some(rotate_cw)}
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
                                let onclick = if z == 0 && i != 4 && cubie.is_none() && !game_frozen {
                                    let color = player_to_move_color.clone();
                                    Some(apply_move.reform(move |_| Move::Drop {
                                        color,
                                        column: (i / 3, i % 3),
                                    }))
                                } else {
                                    None
                                };

                                let drop_move = Move::Drop {
                                    color: player_to_move_color.clone(),
                                    column: (i / 3, i % 3),
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
                                        style={format!("--slot-color: {color}; --highlight-color: {player_to_move_color};")}
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
                            class={classes!("rotate-button", if is_hovered_cw { "highlighted" } else { "" })}
                            style={if is_hovered_cw { format!("--highlight-color: {};", player_to_move_color) } else { String::new() }}
                            onclick={apply_move.reform(move |_| rotate_cw)}
                            disabled={game_frozen || props.game_state.last_move == Some(rotate_ccw)}
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
                disabled={game_frozen || props.game_state.last_move == Some(Move::Flip)}
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

            <h2 style="text-align: center;">
                {
                    if let Some((winner, _)) = won {
                        format!("{} won!", winner.color)
                    } else {
                        "".to_string()
                    }
                }
            </h2>

            <button
                onclick={{
                    let game_state_handle = game_state_handle.clone();
                    let history_handle = history_handle.clone();
                    Callback::from(move |_| {
                        let mut new_history = (*history_handle).clone();
                        if let Some(prev_state) = new_history.pop() {
                            game_state_handle.set(prev_state);
                            history_handle.set(new_history);
                        }
                    })
                }}
                disabled={(*history_handle).is_empty()}
            >{ "Undo last move" }</button>

            <button
                onclick={{
                    let game_state_handle = game_state_handle.clone();
                    let history_handle = history_handle.clone();
                    Callback::from(move |_| {
                        game_state_handle.set(GameState::new(12, 12));
                        history_handle.set(Vec::new());
                    })
                }}
            >{ "Restart the game" }</button>

        </div>
    }
}
