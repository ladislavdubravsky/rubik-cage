use crate::{
    core::game::GameState,
    search::{
        self, Evaluation, EvaluationMap,
        bounded::{Budget, Search},
        packed::Space,
        retrograde::{self, Limits, SolveError},
    },
};
use std::{cell::RefCell, collections::HashSet};
use yew_agent::prelude::oneshot;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct EvaluationTaskSpec {
    pub state: GameState,
    pub request_id: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct EvaluationTaskResult {
    pub request_id: u64,
    /// Every entry is exact, even when other moves remain unknown.
    pub values: EvaluationMap,
    pub problem: Option<String>,
}

// A single worker owns both caches; packed bounds never cross inventory/color spaces.
thread_local! {
    static KNOWN: RefCell<EvaluationMap> = RefCell::new(super::evaluation::precomputed());
    static SEARCH: RefCell<Option<Search>> = const { RefCell::new(None) };
}

fn evaluate(
    state: &GameState,
    known: &mut EvaluationMap,
    retained: &mut Option<Search>,
) -> Result<(EvaluationMap, bool), String> {
    let space = Space::new(state).map_err(str::to_owned)?;
    let mut requested = vec![*state];
    let mut seen = HashSet::from([state.position_key()]);
    for m in state.legal_moves() {
        let mut child = *state;
        child.apply_move(m).unwrap();
        if seen.insert(child.position_key()) {
            requested.push(child);
        }
    }
    let missing = |s: &GameState, values: &EvaluationMap| {
        Evaluation::terminal(s).is_none() && !values.contains_key(&s.position_key())
    };
    let mut incoming = EvaluationMap::new();
    // Tiny complete graphs also prove draws, without spending a horizon budget first.
    if space.totals.iter().map(|&n| usize::from(n)).sum::<usize>() > 5 {
        if retained
            .as_ref()
            .is_none_or(|s| s.proof.space != space || s.proof.bounds.len() >= 250_000)
        {
            *retained = Some(Search::new(
                state,
                Budget {
                    max_positions: 250_000,
                    max_calls: 1_000_000,
                },
            )?);
        }
        let search = retained.as_mut().unwrap();
        search.budget.max_calls = search.calls.saturating_add(1_000_000);
        for s in &requested {
            if missing(s, known) {
                match search.exact(s, 18) {
                    Ok(Some(value)) => {
                        incoming.insert(s.position_key(), value);
                    }
                    Ok(None) => (),
                    Err(_) => break, // Completed bounds remain valid; unfinished claims are absent.
                }
            }
        }
        // Custom inventories have no bundled table: certify closed draw strategies
        // from the explored positions as well as finite-horizon wins.
        search.proof.close_safety();
        search::merge_exact(&mut incoming, search.proof.exact_values())?;
        search::merge_exact(known, incoming.clone())?;
    }
    if requested.iter().any(|s| missing(s, known)) {
        // Expand the displayed root even when cached: we need its move values too.
        let key = state.position_key();
        let previous = known.remove(&key);
        let graph = retrograde::solve(
            state,
            known,
            Limits {
                max_states: 20_000,
                max_edges: 300_000,
            },
        );
        if let Some(value) = previous {
            known.insert(key, value);
        }
        match graph {
            Ok(solution) => {
                let mut fresh = solution.values;
                search::merge_exact(known, fresh.clone())?;
                // Avoid duplicating the horizon results in this response.
                fresh.retain(|k, _| !incoming.contains_key(k));
                search::merge_exact(&mut incoming, fresh)?;
            }
            Err(SolveError::LimitReached(_)) => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    let complete = requested.iter().all(|s| !missing(s, known));
    Ok((incoming, complete))
}

#[oneshot]
pub async fn EvaluationTask(spec: EvaluationTaskSpec) -> EvaluationTaskResult {
    let result = KNOWN.with(|known| {
        SEARCH.with(|search| {
            let mut known = known.borrow_mut();
            let result = evaluate(&spec.state, &mut known, &mut search.borrow_mut());
            if known.len() > 500_000 {
                *known = super::evaluation::precomputed();
            }
            result
        })
    });
    let (values, problem) = match result {
        Ok((values, true)) => (values, None),
        Ok((values, false)) => (
            values,
            Some("Search incomplete. Unsolved moves remain unknown.".into()),
        ),
        Err(error) => (
            EvaluationMap::new(),
            Some(format!("Evaluation could not be completed: {error}")),
        ),
    };
    EvaluationTaskResult {
        request_id: spec.request_id,
        values,
        problem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_publishes_exact_partial_results_and_reuses_only_matching_spaces() {
        let root = GameState::new(12, 12);
        let mut known = EvaluationMap::new();
        let mut retained = None;
        let (values, _) = evaluate(&root, &mut known, &mut retained).unwrap();
        assert_eq!(
            values[&root.position_key()],
            Evaluation::Win {
                winner: 0,
                plies: 11
            }
        );
        retained.as_ref().unwrap().proof.verify().unwrap();
        let other = GameState::new(4, 2);
        evaluate(&other, &mut known, &mut retained).unwrap();
        assert_eq!(
            retained.as_ref().unwrap().proof.space,
            Space::new(&other).unwrap()
        );
        let (_, complete) = evaluate(&GameState::new(3, 2), &mut known, &mut retained).unwrap();
        assert!(complete);
        assert_eq!(
            known[&GameState::new(3, 2).position_key()],
            Evaluation::Draw
        );
    }
    #[test]
    fn custom_inventory_results_match_complete_graphs_without_assets() {
        let mut known = EvaluationMap::new();
        let mut retained = None;
        for (m, n) in [(0, 0), (3, 0), (0, 3), (1, 1), (3, 2)] {
            let state = GameState::new(m, n);
            let reference =
                retrograde::solve(&state, &EvaluationMap::new(), Limits::default()).unwrap();
            let (values, complete) = evaluate(&state, &mut known, &mut retained).unwrap();
            assert!(complete, "Unsolved small custom game {m},{n}");
            for (key, value) in values {
                assert_eq!(reference.values[&key], value);
            }
            assert_eq!(
                known[&state.position_key()],
                reference.values[&state.position_key()]
            );
        }
    }
}
