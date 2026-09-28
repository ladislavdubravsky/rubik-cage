//! One app-level scheduler owns all evaluation requests and publishes exact values.
use super::agent::{EvaluationTask, EvaluationTaskSpec};
use crate::{
    core::{game::GameState, position::PositionKey},
    search::{self, Evaluation, EvaluationMap, cache::Table, retrograde::SolveError},
};
use std::{cell::RefCell, collections::HashSet, rc::Rc};
use yew::{platform::spawn_local, prelude::*};
use yew_agent::oneshot::use_oneshot_runner;

pub fn precomputed() -> EvaluationMap {
    // Incompatible data never reaches the running map. No legacy eval.bin fallback.
    Table::decode(include_bytes!("../../assets/eval-v1.bin"))
        .map(|t| t.values)
        .unwrap_or_default()
}

#[derive(Clone, PartialEq)]
pub struct EvaluationContext {
    pub values: Rc<RefCell<EvaluationMap>>,
    pub revision: u64,
    pub running: Option<PositionKey>,
    pub problem: Option<(PositionKey, String)>,
}
impl EvaluationContext {
    pub fn get(&self, state: &GameState) -> Option<Evaluation> {
        Evaluation::terminal(state)
            .or_else(|| self.values.borrow().get(&state.position_key()).copied())
    }
    pub fn unknown_label(&self, root: PositionKey) -> &'static str {
        if self.running == Some(root) {
            "Calculating..."
        } else if self.problem.as_ref().is_some_and(|(k, _)| *k == root) {
            "Unknown (search incomplete)"
        } else {
            "Unknown"
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct EvaluationProviderProps {
    pub state: UseStateHandle<GameState>,
    pub children: Children,
}

#[function_component(EvaluationProvider)]
pub fn evaluation_provider(props: &EvaluationProviderProps) -> Html {
    let values = use_mut_ref(precomputed);
    let revision = use_state(|| 0u64);
    let running = use_state(|| None::<PositionKey>);
    let problem = use_state(|| None::<(PositionKey, String)>);
    let attempted = use_mut_ref(HashSet::<PositionKey>::new);
    let serial = use_mut_ref(|| 0u64);
    let runner = use_oneshot_runner::<EvaluationTask>();
    let state = *props.state;
    let key = state.position_key();
    {
        let values = values.clone();
        let revision = revision.clone();
        let running = running.clone();
        let problem = problem.clone();
        use_effect_with((key, *revision, *running), move |_| {
            let missing = state.legal_moves().into_iter().any(|m| {
                let mut child = state;
                child.apply_move(m).unwrap();
                Evaluation::terminal(&child).is_none()
                    && !values.borrow().contains_key(&child.position_key())
            });
            if running.is_none() && missing && attempted.borrow_mut().insert(key) {
                running.set(Some(key));
                problem.set(None);
                *serial.borrow_mut() += 1;
                let request_id = *serial.borrow();
                spawn_local(async move {
                    let response = runner.run(EvaluationTaskSpec { state, request_id }).await;
                    if response.request_id != *serial.borrow() {
                        return;
                    }
                    let error = match response.result {
                        Ok(solution) => {
                            let retained = solution.values.clone();
                            let result = search::merge_exact(&mut values.borrow_mut(), solution.values);
                            if result.is_ok() {
                                attempted.borrow_mut().clear();
                                if values.borrow().len() > 100_000 { *values.borrow_mut() = retained; }
                            }
                            result.err().map(|_| "Conflicting evaluation results; please reload.".to_string())
                        }
                        Err(SolveError::LimitReached(_)) => Some("This position exceeds the search limit. Unsolved moves remain unknown.".into()),
                        Err(_) => Some("Evaluation could not be completed.".into()),
                    };
                    if let Some(message) = error {
                        problem.set(Some((key, message)));
                    }
                    revision.set(*revision + 1);
                    running.set(None);
                });
            }
            || ()
        });
    }
    let context = EvaluationContext {
        values,
        revision: *revision,
        running: *running,
        problem: (*problem).clone(),
    };
    let message = problem
        .as_ref()
        .filter(|(k, _)| *k == key)
        .map(|(_, message)| message.clone());
    html! {
        <ContextProvider<EvaluationContext> context={context}>
            { for props.children.iter() }
            if let Some(message) = message { <p role="status">{message}</p> }
        </ContextProvider<EvaluationContext>>
    }
}
