use crate::{
    app::{
        evaluation::EvaluationContext,
        hovered_move::use_hovered_move,
        utils::{self, use_apply_move_callback},
    },
    core::{
        cubie::Cubie,
        game::{GameState, Player},
    },
    search::Evaluation,
};
use std::rc::Rc;
use web_sys::window;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PlayerPanelProps {
    pub player: Player,
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
    pub selected_colors: UseStateHandle<[Option<Cubie>; 2]>,
}

pub(crate) fn eval_to_string(eval: Option<Evaluation>, player_id: u8, unknown: &str) -> String {
    match eval {
        Some(Evaluation::Win { winner, plies }) => format!(
            "{} in {}",
            if winner == player_id { "Win" } else { "Loss" },
            u64::from(plies) + 1
        ),
        Some(Evaluation::Draw) => "Draw".into(),
        None => unknown.into(),
    }
}

#[function_component(PlayerPanel)]
pub fn player_panel(props: &PlayerPanelProps) -> Html {
    let is_turn = props.game_state.player_to_move.id == props.player.id;

    // Persist move_list_visible state in localStorage per player
    let player_id = props.player.id;
    let storage_key = format!("move_list_visible_{}", player_id);
    let move_list_visible = use_state(|| {
        window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|storage| storage.get_item(&storage_key).ok().flatten())
            .map(|v| v == "true")
            .unwrap_or(false)
    });
    let eval = use_context::<EvaluationContext>().expect("EvaluationProvider");

    let is_won = props.game_state.outcome().is_some();
    let apply_move =
        use_apply_move_callback(props.game_state.clone(), props.history.clone(), is_won);

    let selected = utils::selected_color(
        &props.game_state,
        props.player.id,
        props.selected_colors[props.player.id as usize],
    );
    let highlight_color = utils::slot_to_css(selected);
    let multicolor = props.game_state.single_colors().is_none();

    let moves = if props.game_state.outcome().is_some() {
        Vec::new() // Don't show further moves if game is finished
    } else {
        utils::sort_moves_by_evaluation(props.game_state.legal_moves(), &props.game_state, &eval)
    };
    let (hovered_move, set_hovered_move) = use_hovered_move();

    // Save move_list_visible to localStorage on change
    {
        let move_list_visible = move_list_visible.clone();
        let storage_key = storage_key.clone();
        use_effect_with(move_list_visible.clone(), move |visible| {
            if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
                storage
                    .set_item(&storage_key, if **visible { "true" } else { "false" })
                    .ok();
            }
            || ()
        });
    }

    html! {
        <div class={classes!("player-panel", if is_turn { "active-turn" } else { "" })}>
            <h2>{ format!("Player {}", props.player.id + 1) }</h2>
            <p>{ "Remaining cubies:" }</p>
            <div class="color-reserves" role="group" aria-label={format!("Player {} colors", props.player.id + 1)}>
                { for props.game_state.colors_for(props.player.id).map(|color| {
                    let count = props.game_state.remaining(color);
                    let select = {
                        let selected_colors = props.selected_colors.clone();
                        let set_hovered_move = set_hovered_move.clone();
                        let id = props.player.id as usize;
                        Callback::from(move |_| {
                            let mut next = *selected_colors;
                            next[id] = Some(color);
                            selected_colors.set(next);
                            set_hovered_move.emit(None);
                        })
                    };
                    html! {
                        <div class="color-reserve" data-color={color.to_string()}>
                            if multicolor {
                                <button type="button" class="color-selector" data-color={color.to_string()}
                                    aria-label={format!("Select {color}, {count} remaining")}
                                    aria-pressed={(selected == Some(color)).to_string()}
                                    disabled={!is_turn || is_won || count == 0} onclick={select}>
                                    <span class={classes!("cubie-icon", color.to_string())} aria-hidden="true" />
                                    {format!("{color}: {count}")}
                                </button>
                            }
                            <div class="cubies-remaining" aria-label={format!("{count} {color} cubies remaining")}>
                                {for (0..count).map(|i| html! { <div class={classes!("cubie-icon", color.to_string())} key={i} /> })}
                            </div>
                        </div>
                    }
                }) }
            </div>
            if multicolor && is_turn && !is_won {
                <p class="selection-status">{selected.map(|c| format!("Place {c}: choose a column.")).unwrap_or_else(|| "No cubies left. Rotate a layer or flip the cage.".into())}</p>
            }
            <label>
                <input
                    type="checkbox"
                    checked={*move_list_visible}
                    onchange={{
                        let move_list_visible = move_list_visible.clone();
                        move |e: web_sys::Event| {
                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                            move_list_visible.set(input.checked());
                        }
                    }}
                />
                { "Show move evaluation" }
            </label>
            {
                if *move_list_visible && is_turn {
                    html! {
                        <ul class="move-list">
                            { for moves.iter().map(|mv| {
                                let mut new_state = (*props.game_state).clone();
                                new_state.apply_move_normalize(mv.clone()).unwrap();
                                let label = eval_to_string(eval.get(&new_state), props.game_state.player_to_move.id, eval.unknown_label(props.game_state.position_key()));
                                let is_hovered = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == mv);
                                let mv = mv.clone();
                                let highlight_color = match mv { crate::core::r#move::Move::Drop { color, .. } => utils::slot_to_css(Some(color)), _ => highlight_color };
                                html! {
                                    <li
                                        class={if is_hovered { "move-highlighted" } else { "" }}
                                        style={if is_hovered { format!("--highlight-color: {};", highlight_color) } else { String::new() }}
                                        onclick={apply_move.reform(move |_| mv.clone())}
                                        onmouseenter={ {
                                            let set_hovered_move = set_hovered_move.clone();
                                            let mv = Rc::new(mv.clone());
                                            move |_| set_hovered_move.emit(Some(mv.clone()))
                                        }}
                                        onmouseleave={ {
                                            let set_hovered_move = set_hovered_move.clone();
                                            move |_| set_hovered_move.emit(None)
                                        }}
                                    >
                                        { format!("{}: {}", utils::move_label(&props.game_state, mv), label) }
                                    </li>
                                }
                            })}
                        </ul>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_include_the_selected_move() {
        assert_eq!(
            eval_to_string(
                Some(Evaluation::Win {
                    winner: 0,
                    plies: 0
                }),
                0,
                "Unknown"
            ),
            "Win in 1"
        );
        assert_eq!(
            eval_to_string(
                Some(Evaluation::Win {
                    winner: 0,
                    plies: 7
                }),
                1,
                "Unknown"
            ),
            "Loss in 8"
        );
        assert_eq!(eval_to_string(Some(Evaluation::Draw), 0, "Unknown"), "Draw");
        assert_eq!(eval_to_string(None, 0, "Unknown"), "Unknown");
    }
}
