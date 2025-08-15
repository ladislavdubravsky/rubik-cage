use crate::{
    EVAL,
    core::game::{GameState, Player},
};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PlayerPanelProps {
    pub player: Player,
    pub game_state: UseStateHandle<GameState>,
}

fn eval_to_string(eval: Option<&isize>, player_id: u8) -> String {
    match (eval, player_id) {
        (Some(1), 0) => "Win".to_string(),
        (Some(1), 1) => "Lose".to_string(),
        (Some(-1), 0) => "Lose".to_string(),
        (Some(-1), 1) => "Win".to_string(),
        (Some(0), _) => "Draw".to_string(),
        _ => "Unknown".to_string(),
    }
}

#[function_component(PlayerPanel)]
pub fn player_panel(props: &PlayerPanelProps) -> Html {
    let is_turn = props.game_state.player_to_move.id == props.player.id;
    let move_list_visible = use_state(|| false);

    let cubies = (0..props.game_state.remaining_cubies[props.player.id as usize]).map(|i| {
        html! {
            <div class={classes!("cubie-icon", props.player.color.to_string())} key={i} />
        }
    });

    let moves = props.game_state.legal_moves();

    html! {
        <div class={classes!("player-panel", if is_turn { "active-turn" } else { "" })}>
            <h2>{ format!("Player {}", props.player.id + 1) }</h2>
            <p>{ "Remaining cubies:" }</p>
            <div class="cubies-remaining">
                { for cubies }
            </div>
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
                                // TODO: calculate missing evaluations
                                let eval = EVAL.get(&new_state.zobrist_hash);
                                let eval = eval_to_string(eval, props.game_state.player_to_move.id);
                                html! {
                                    <li>{ format!("{}: {}", mv, eval) }</li>
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
