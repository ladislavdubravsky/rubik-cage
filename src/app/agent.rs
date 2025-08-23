use crate::{core::game::GameState, search};
use std::collections::HashMap;
use yew_agent::prelude::oneshot;

#[oneshot]
pub async fn EvaluationTask(state: GameState) -> HashMap<u64, isize> {
    search::naive::evaluate(&state, false)
}
