use crate::{
    core::game::GameState,
    search::{
        self,
        naive::{Evaluation, SearchMode},
    },
};
use std::collections::HashMap;
use yew_agent::prelude::oneshot;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct EvaluationTaskSpec {
    pub state: GameState,
    pub search_mode: SearchMode,
}

#[oneshot]
pub async fn EvaluationTask(spec: EvaluationTaskSpec) -> HashMap<u64, Evaluation> {
    search::naive::evaluate(&spec.state, spec.search_mode)
}
