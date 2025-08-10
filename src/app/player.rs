use crate::core::game::{GameState, Player};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PlayerPanelProps {
    pub player: Player,
    pub game_state: UseStateHandle<GameState>,
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
                            { for moves.iter().map(|mv| html! {
                                <li>{ format!("{} ({})", mv, "-") }</li>
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
