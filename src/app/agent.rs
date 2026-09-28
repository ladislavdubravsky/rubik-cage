use crate::{
    core::game::GameState,
    search::{
        self, EvaluationMap,
        retrograde::{self, Limits, Solution, SolveError},
    },
};
use std::cell::RefCell;
use yew_agent::prelude::oneshot;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct EvaluationTaskSpec {
    pub state: GameState,
    pub request_id: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct EvaluationTaskResult {
    pub request_id: u64,
    pub result: Result<Solution, SolveError>,
}

// The worker retains exact boundaries between requests instead of copying the
// browser's growing table into every task. Requests are serialized by the provider.
thread_local! {
    static KNOWN: RefCell<EvaluationMap> = RefCell::new(super::evaluation::precomputed());
}

#[oneshot]
pub async fn EvaluationTask(spec: EvaluationTaskSpec) -> EvaluationTaskResult {
    let result = KNOWN.with(|known| {
        let mut known = known.borrow_mut();
        let root = spec.state.position_key();
        // Expand the displayed root even if its own value is known: the UI needs
        // evaluations for all its moves, not just another copy of the root value.
        let previous = known.remove(&root);
        let result = retrograde::solve(
            &spec.state,
            &known,
            Limits {
                max_states: 20_000,
                max_edges: 300_000,
            },
        );
        if let Some(value) = previous {
            known.insert(root, value);
        }
        if let Ok(solution) = &result {
            search::merge_exact(&mut known, solution.values.clone())
                .map_err(|_| SolveError::ConflictingTerminal)?;
            // Bounded reuse; discarding exact entries only makes future work unknown.
            if known.len() > 100_000 {
                *known = solution.values.clone();
            }
        }
        result
    });
    EvaluationTaskResult {
        request_id: spec.request_id,
        result,
    }
}
