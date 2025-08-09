use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PlayerPanelProps {
    pub player_id: u8,
    pub is_turn: bool,
}

#[function_component(PlayerPanel)]
pub fn player_panel(props: &PlayerPanelProps) -> Html {
    let color = if props.player_id == 1 { "blue" } else { "red" };
    let move_list_visible = use_state(|| false);

    let cubies = (0..12).map(|i| {
        html! {
            <div class={classes!("cubie-icon", color)} key={i} />
        }
    });

    let moves = vec![
        ("Drop A1", "Blue Wins"),
        ("Rotate Up", "Draw"),
        ("Flip", "Red Wins"),
    ];

    html! {
        <div class={classes!("player-panel", if props.is_turn { "active-turn" } else { "" })}>
            <h2>{ format!("Player {}", props.player_id) }</h2>
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
                { "Show moves with evaluation" }
            </label>
            {
                if *move_list_visible {
                    html! {
                        <ul class="move-list">
                            { for moves.iter().map(|(mv, eval)| html! {
                                <li>{ format!("{} ({})", mv, eval) }</li>
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
