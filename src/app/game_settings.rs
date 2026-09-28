use crate::{core::game::GameState, search::packed::Space};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct GameSettingsProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
}

#[function_component(GameSettings)]
pub fn game_settings(props: &GameSettingsProps) -> Html {
    let open = use_state(|| false);
    let draft = use_state(|| ["12".to_owned(), "12".to_owned()]);
    let toggle = {
        let open = open.clone();
        let draft = draft.clone();
        let state = *props.game_state;
        Callback::from(move |_| {
            if !*open {
                // Show starting stocks, including pieces already on the board.
                let totals = Space::new(&state).expect("Validated game state").totals;
                draft.set(totals.map(|n| n.min(12).to_string()));
            }
            open.set(!*open);
        })
    };
    let sizes = draft[0]
        .parse::<u8>()
        .ok()
        .zip(draft[1].parse::<u8>().ok())
        .filter(|&(m, n)| m <= 12 && n <= 12);
    let start = {
        let open = open.clone();
        let game_state = props.game_state.clone();
        let history = props.history.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if let Some((m, n)) = sizes {
                let mut initial = GameState::new(m, n);
                initial.players = game_state.players;
                initial.player_to_move = initial.players[0];
                game_state.set(initial);
                history.set(Vec::new());
                open.set(false);
            }
        })
    };
    let cancel = {
        let open = open.clone();
        Callback::from(move |_| open.set(false))
    };
    let escape = {
        let open = open.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                open.set(false);
            }
        })
    };

    html! {
        <div class="game-settings">
            <button type="button" class="settings-toggle" onclick={toggle}
                aria-expanded={open.to_string()} aria-controls="game-settings-panel">
                { "Game settings" }
            </button>
            if *open {
                <form id="game-settings-panel" class="settings-panel" onsubmit={start} onkeydown={escape}>
                    <p>{ "Starting cubies (0–12 each)" }</p>
                    <div class="settings-fields">
                        { for (0..2).map(|id| {
                            let oninput = {
                                let draft = draft.clone();
                                Callback::from(move |event: InputEvent| {
                                    let input: HtmlInputElement = event.target_unchecked_into();
                                    let mut next = (*draft).clone();
                                    next[id] = input.value();
                                    draft.set(next);
                                })
                            };
                            html! {
                                <label>
                                    { format!("Player {}", id + 1) }
                                    <input type="number" name={format!("p{}-cubies", id + 1)}
                                        min="0" max="12" step="1" required=true
                                        value={draft[id].clone()} {oninput} />
                                </label>
                            }
                        }) }
                    </div>
                    <div class="settings-actions">
                        <button type="submit" class="control-button" disabled={sizes.is_none()}>{ "Start new game" }</button>
                        <button type="button" class="settings-toggle" onclick={cancel}>{ "Cancel" }</button>
                    </div>
                </form>
            }
        </div>
    }
}
