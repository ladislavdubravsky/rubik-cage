use crate::{core::game::GameState, search::{self, naive::Evaluation}};
use std::collections::HashMap;
use yew_agent::prelude::oneshot;

#[oneshot]
pub async fn EvaluationTask(state: GameState) -> HashMap<u64, Evaluation> {
    search::naive::evaluate(&state, false)
}
