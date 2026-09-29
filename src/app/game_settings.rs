use crate::app::ai::{AiContext, AiInteraction};
use crate::core::{cubie::Cubie, game::GameState};
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct GameSettingsProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
}

#[function_component(GameSettings)]
pub fn game_settings(props: &GameSettingsProps) -> Html {
    let ai = use_context::<AiContext>();
    let open = use_state(|| false);
    let draft = use_state(|| GameState::new(12, 12));
    let stocks = use_state(|| {
        GameState::new(12, 12)
            .remaining_cubies
            .map(|n| n.to_string())
    });
    let toggle = {
        let ai = ai.clone();
        let open = open.clone();
        let draft = draft.clone();
        let stocks = stocks.clone();
        let state = *props.game_state;
        Callback::from(move |_| {
            if !*open {
                if let Some(ai) = &ai {
                    ai.0.emit(AiInteraction::Reset);
                }
                let initial = state.restarted();
                stocks.set(initial.remaining_cubies.map(|n| n.to_string()));
                draft.set(initial);
            }
            open.set(!*open);
        })
    };
    let preset = {
        let ai = ai.clone();
        let draft = draft.clone();
        let stocks = stocks.clone();
        Callback::from(move |event: Event| {
            if let Some(ai) = &ai {
                ai.0.emit(AiInteraction::Reset);
            }
            let input: HtmlSelectElement = event.target_unchecked_into();
            let initial = if input.value() == "multi" {
                GameState::multicolor()
            } else {
                GameState::new(12, 12)
            };
            stocks.set(initial.remaining_cubies.map(|n| n.to_string()));
            draft.set(initial);
        })
    };
    let multi = draft.single_colors().is_none();
    let candidate = (|| {
        let mut remaining = [0; 6];
        for c in Cubie::ALL {
            if draft.owner(c).is_some() {
                let n = stocks[c as usize].parse::<u8>().ok()?;
                if n > 12 {
                    return None;
                }
                remaining[c as usize] = n;
            }
        }
        GameState::with_colors(draft.color_owners, remaining).ok()
    })();
    let start = {
        let ai = ai.clone();
        let open = open.clone();
        let game_state = props.game_state.clone();
        let history = props.history.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if let Some(initial) = candidate {
                if let Some(ai) = &ai {
                    ai.0.emit(AiInteraction::Reset);
                }
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
            <button type="button" class="settings-toggle" onclick={toggle} aria-expanded={open.to_string()} aria-controls="game-settings-panel">{"Game settings"}</button>
            if *open {
                <form id="game-settings-panel" class="settings-panel" onsubmit={start} onkeydown={escape}>
                    <label>{"Game preset"}
                        <select name="game-preset" onchange={preset}>
                            <option value="single" selected={!multi}>{"One color per player"}</option>
                            <option value="multi" selected={multi}>{"Three colors each · three cubies per color"}</option>
                        </select>
                    </label>
                    <p>{if multi { "Starting cubies per color (0–12); at most 24 per player." } else { "Starting cubies (0–12 each)" }}</p>
                    if multi { <p>{"Win with three of one color in a line. Mixed colors do not win."}</p> }
                    <div class="settings-fields">
                        {for (0..2u8).map(|id| html! {
                            <fieldset>
                                <legend>{format!("Player {}", id + 1)}</legend>
                                {for draft.colors_for(id).map(|color| {
                                    let oninput = { let ai = ai.clone(); let stocks = stocks.clone(); Callback::from(move |event: InputEvent| {
                                        if let Some(ai) = &ai { ai.0.emit(AiInteraction::Reset); }
                                        let input: HtmlInputElement = event.target_unchecked_into();
                                        let mut next = (*stocks).clone(); next[color as usize] = input.value(); stocks.set(next);
                                    }) };
                                    html! { <label>
                                        {color.to_string()}
                                        <input type="number" name={if multi { format!("p{}-{}-cubies", id + 1, color) } else { format!("p{}-cubies", id + 1) }}
                                            min="0" max="12" step="1" required=true value={stocks[color as usize].clone()} {oninput} />
                                    </label> }
                                })}
                            </fieldset>
                        })}
                    </div>
                    if candidate.is_none() { <p role="alert">{"Enter whole counts from 0 to 12, with at most 24 cubies per player."}</p> }
                    <div class="settings-actions">
                        <button type="submit" class="control-button" disabled={candidate.is_none()}>{"Start new game"}</button>
                        <button type="button" class="settings-toggle" onclick={cancel}>{"Cancel"}</button>
                    </div>
                </form>
            }
        </div>
    }
}
