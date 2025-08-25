use crate::{app::utils::RELOAD_FLAG_KEY, core::game::GameState};
use bincode::{decode_from_slice, encode_to_vec};
use web_sys::{
    HtmlInputElement, Url, js_sys,
    wasm_bindgen::{JsCast, prelude::Closure},
};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct GameControlProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
}

#[function_component(GameControl)]
pub fn game_control(props: &GameControlProps) -> Html {
    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let undo = Callback::from(move |_| {
        let mut new_history = (*history_handle).clone();
        if let Some(prev_state) = new_history.pop() {
            game_state_handle.set(prev_state);
            history_handle.set(new_history);
        }
    });

    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let restart = Callback::from(move |_| {
        game_state_handle.set(GameState::new(12, 12));
        history_handle.set(Vec::new());
    });

    let game_state_handle = props.game_state.clone();
    let export = Callback::from(move |_| {
        // Set reload flag in localStorage before export-triggered reload
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            storage.set_item(RELOAD_FLAG_KEY, "true").ok();
        }
        let state = &*game_state_handle;
        let bin = encode_to_vec(state, bincode::config::standard()).unwrap();
        let uint8_array = js_sys::Uint8Array::from(bin.as_slice());
        let blob =
            web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&uint8_array)).unwrap();
        let url = Url::create_object_url_with_blob(&blob).unwrap();
        let window = web_sys::window().unwrap();
        window.open_with_url(&url).unwrap();
        Url::revoke_object_url(&url).unwrap();
    });

    let game_state_handle = props.game_state.clone();
    let history_handle = props.history.clone();
    let import = Callback::from(move |_| {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let input = document.create_element("input").unwrap();
        input.set_attribute("type", "file").unwrap();
        input.set_attribute("accept", ".bin").unwrap();
        let input_html: HtmlInputElement = input.unchecked_into();
        let game_state_handle = game_state_handle.clone();
        let history_handle = history_handle.clone();
        let window = window.clone();
        let input_html_handle = input_html.clone();
        let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let files = input_html_handle.files();
            if let Some(files) = files {
                if let Some(file) = files.get(0) {
                    let fr = web_sys::FileReader::new().unwrap();
                    let fr_clone = fr.clone();
                    let game_state_handle = game_state_handle.clone();
                    let history_handle = history_handle.clone();
                    let window = window.clone();
                    let onload = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                        let result = fr_clone.result().unwrap();
                        let array = js_sys::Uint8Array::new(&result);
                        let mut vec = vec![0u8; array.length() as usize];
                        array.copy_to(&mut vec[..]);
                        if let Ok((state, _)) =
                            decode_from_slice::<GameState, _>(&vec, bincode::config::standard())
                        {
                            game_state_handle.set(state);
                            history_handle.set(Vec::new());
                        } else {
                            window.alert_with_message("Failed to import position").ok();
                        }
                    }) as Box<dyn FnMut(_)>);
                    fr.set_onload(Some(onload.as_ref().unchecked_ref()));
                    fr.read_as_array_buffer(&file).unwrap();
                    onload.forget();
                }
            }
        }) as Box<dyn FnMut(_)>);
        input_html.set_onchange(Some(closure.as_ref().unchecked_ref()));
        document.body().unwrap().append_child(&input_html).unwrap();
        input_html.click();
        document.body().unwrap().remove_child(&input_html).unwrap();
        closure.forget();
    });

    html! {
        <div class="game-control">
            <button class="control-button" onclick={undo} disabled={(*props.history).is_empty()}>{ "Undo last move" }</button>
            <button class="control-button" onclick={restart}>{ "Restart the game" }</button>
            <button class="control-button" onclick={export}>{ "Export position" }</button>
            <button class="control-button" onclick={import}>{ "Import position" }</button>
        </div>
    }
}
