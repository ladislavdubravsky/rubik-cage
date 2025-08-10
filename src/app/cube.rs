use crate::core::game::GameState;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CubeProps {
    pub game_state: UseStateHandle<GameState>,
}

#[function_component(Cube)]
pub fn cube(props: &CubeProps) -> Html {
    html! {
        <div class="cage">
            { for (0..3).rev().map(|z| html! {
                <div class="layer">
                    <div class="grid">
                        { for (0..9).map(|i| {
                            let cubie = props.game_state.cage.grid[i / 3][i % 3][z];
                            let color = cubie.as_ref().map(|c| c.to_string()).unwrap_or("#444".into());
                            html! {
                                <div class="slot" style={format!("--slot-color: {color};")} />
                            }
                        }) }
                    </div>
                </div>
            }) }
        </div>
    }
}
