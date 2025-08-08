mod app;
mod core;
mod search;

use app::cube::Cube;
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <div class="app">
            <h1>{ "Rubik's Cage" }</h1>
            <Cube />
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
