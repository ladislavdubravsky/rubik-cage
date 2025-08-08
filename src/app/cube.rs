use yew::prelude::*;

#[function_component(Cube)]
pub fn cube() -> Html {
    let layer_names = ["Up", "Equator", "Down"];

    html! {
        <div class="cube">
            { for layer_names.iter().enumerate().map(|(layer_idx, &name)| html! {
                <div class="layer">
                    <div class="grid">
                        { for (0..9).map(|i| html! {
                            <div class="square" key={layer_idx * 10 + i}>{ i }</div>
                        }) }
                    </div>
                </div>
            }) }
        </div>
    }
}
