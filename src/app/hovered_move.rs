use crate::core::r#move::Move;
use std::rc::Rc;
use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct HoveredMove(pub Option<Rc<Move>>);

#[hook]
pub fn use_hovered_move() -> (HoveredMove, Callback<Option<Rc<Move>>>) {
    use_context::<(HoveredMove, Callback<Option<Rc<Move>>>)>()
        .expect("HoveredMove context not found")
}

#[derive(Properties, PartialEq)]
pub struct HoveredMoveProviderProps {
    pub children: Children,
}

#[function_component(HoveredMoveProvider)]
pub fn hovered_move_provider(props: &HoveredMoveProviderProps) -> Html {
    let hovered = use_state(|| HoveredMove(None));
    let set_hovered = {
        let hovered = hovered.clone();
        Callback::from(move |mv: Option<Rc<Move>>| hovered.set(HoveredMove(mv)))
    };
    html! {
        <ContextProvider<(HoveredMove, Callback<Option<Rc<Move>>>)> context={( (*hovered).clone(), set_hovered )}>
            { for props.children.iter() }
        </ContextProvider<(HoveredMove, Callback<Option<Rc<Move>>>)>>
    }
}
