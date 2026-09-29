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
    pub batch: u8,
    pub ai: Option<AiTaskSpec>,
}

/// Approximate advice shares the exact worker, but never its result map.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct AiTaskSpec {
    pub session_id: u64,
    pub max_millis: u32,
    /// Only already-proved root/child values; never an approximate score.
    pub known: EvaluationMap,
}

struct AiSession {
    id: u64,
    state: GameState,
    known: EvaluationMap,
    requested: Vec<GameState>,
    oracle: Option<search::general::Search>,
    search: Option<search::ai::Search>,
}

pub const AUTOMATIC_BATCHES: u8 = 8;
pub const MAX_BATCHES: u8 = 32;

pub fn batch_limits(state: &GameState) -> (u8, u8) {
    if state.single_colors().is_some() {
        (AUTOMATIC_BATCHES, MAX_BATCHES)
    } else {
        (32, 128)
    }
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TaskStatus {
    Complete,
    Continuing,
    Paused { reason: String, can_resume: bool },
    Failed(String),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct EvaluationTaskResult {
    pub request_id: u64,
    /// Every entry is exact, even when other moves remain unknown.
    pub values: EvaluationMap,
    pub status: TaskStatus,
    pub ai: Option<search::ai::Analysis>,
}

// One worker owns exact and approximate caches; their values and namespaces stay separate.
thread_local! {
    static KNOWN: RefCell<EvaluationMap> = RefCell::new(super::evaluation::precomputed());
    static AI: RefCell<Option<AiSession>> = const { RefCell::new(None) };
    static GENERAL: RefCell<Option<search::general::Search>> = const { RefCell::new(None) };
    static SEARCH: RefCell<Option<Search>> = const { RefCell::new(None) };
}

fn merge_new(known: &mut EvaluationMap, values: EvaluationMap) -> Result<EvaluationMap, String> {
    let fresh = values
        .iter()
        .filter(|(k, v)| known.get(k) != Some(v))
        .map(|(&k, &v)| (k, v))
        .collect();
    search::merge_exact(known, values)?;
    Ok(fresh)
}

fn evaluate(
    state: &GameState,
    known: &mut EvaluationMap,
    retained: &mut Option<Search>,
    batch: u8,
) -> Result<(EvaluationMap, bool), String> {
    let space = Space::new(state).map_err(str::to_owned)?;
    let batch = batch.min(MAX_BATCHES - 1);
    let capacity = 250_000 * (1 + usize::from(batch / AUTOMATIC_BATCHES));
    let horizon = 18 + 2 * batch;
    let mut requested = vec![*state];
    let mut seen = HashSet::from([state.position_key()]);
    for m in state.legal_moves() {
        let mut child = *state;
        child.apply_move(m).unwrap();
        if seen.insert(child.position_key()) {
            requested.push(child);
        }
    }
    // Round-robin the requested moves so one expensive query cannot monopolize
    // every continuation. Completed subqueries stay in the same transposition table.
    let offset = usize::from(batch) % requested.len();
    requested.rotate_left(offset);
    let missing = |s: &GameState, values: &EvaluationMap| {
        Evaluation::terminal(s).is_none() && !values.contains_key(&s.position_key())
    };
    let mut incoming = EvaluationMap::new();
    for s in &requested {
        if missing(s, known)
            && let Some(value) = search::lookup_exact(known, s)
        {
            incoming.insert(s.position_key(), value);
        }
    }
    search::merge_exact(known, incoming.clone())?;
    if requested.iter().all(|s| !missing(s, known)) {
        // The browser can have evicted entries that the worker still retains.
        // Always return the requested values, even when no computation is needed.
        for s in &requested {
            if let Some(&value) = known.get(&s.position_key()) {
                incoming.insert(s.position_key(), value);
            }
        }
        return Ok((incoming, true));
    }
    // Tiny complete graphs also prove draws, without spending a horizon budget first.
    if space.totals.iter().map(|&n| usize::from(n)).sum::<usize>() > 5 {
        if retained.as_ref().is_none_or(|s| s.proof.space != space) {
            *retained = Some(Search::new(
                state,
                Budget {
                    max_positions: capacity,
                    max_calls: 250_000,
                },
            )?);
        }
        let search = retained.as_mut().unwrap();
        search.budget.max_positions = search.budget.max_positions.max(capacity);
        search.budget.max_calls = search.calls.saturating_add(250_000);
        for s in &requested {
            if missing(s, known) {
                match search.exact(s, horizon) {
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
        incoming = merge_new(known, search::include_player_swaps(incoming)?)?;
    }
    if requested.iter().any(|s| missing(s, known))
        && (batch == 0 || !incoming.is_empty() || (batch + 1) % AUTOMATIC_BATCHES == 0)
    {
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
                let fresh = merge_new(known, search::include_player_swaps(solution.values)?)?;
                search::merge_exact(&mut incoming, fresh)?;
            }
            Err(SolveError::LimitReached(_)) => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    let complete = requested.iter().all(|s| !missing(s, known));
    for s in &requested {
        if let Some(&value) = known.get(&s.position_key()) {
            incoming.insert(s.position_key(), value);
        }
    }
    Ok((incoming, complete))
}

fn status_after_batch(complete: bool, batch: u8, at_capacity: bool) -> TaskStatus {
    status_with_limits(complete, batch, at_capacity, AUTOMATIC_BATCHES, MAX_BATCHES)
}

fn status_with_limits(
    complete: bool,
    batch: u8,
    at_capacity: bool,
    automatic: u8,
    maximum: u8,
) -> TaskStatus {
    let batch = batch.min(maximum - 1);
    if complete {
        return TaskStatus::Complete;
    }
    if !at_capacity && (batch + 1) % automatic != 0 && batch + 1 < maximum {
        TaskStatus::Continuing
    } else {
        TaskStatus::Paused {
            reason: if at_capacity {
                "Search paused at the memory limit. Unsolved moves remain unknown."
            } else {
                "Search paused after its work budget. Unsolved moves remain unknown."
            }
            .into(),
            can_resume: (batch / automatic + 1) * automatic < maximum,
        }
    }
}

fn evaluate_general(
    state: &GameState,
    retained: &mut Option<search::general::Search>,
    batch: u8,
) -> Result<(EvaluationMap, TaskStatus), String> {
    if retained.as_ref().is_none_or(|s| !s.matches(state)) {
        *retained = Some(search::general::Search::new(state)?);
    }
    let (automatic, maximum) = batch_limits(state);
    let batch = batch.min(maximum - 1);
    let allowance = 1 + usize::from(batch / automatic);
    let result = retained.as_mut().unwrap().run(
        state,
        search::general::Budget {
            max_steps: 4000,
            max_records: 25_000 * allowance,
            max_bytes: 16 * 1024 * 1024 * allowance,
            max_horizon: 4 + 2 * allowance as u16,
        },
    )?;
    let mut status = status_with_limits(
        result.complete,
        batch,
        result.at_capacity,
        automatic,
        maximum,
    );
    if !result.complete && result.exhausted {
        status = TaskStatus::Paused {
            reason: "Search paused at the depth limit. Unsolved moves remain unknown.".into(),
            can_resume: (batch / automatic + 1) * automatic < maximum,
        };
    }
    Ok((result.values, status))
}

fn evaluate_ai(
    state: &GameState,
    spec: AiTaskSpec,
    retained: &mut Option<AiSession>,
) -> Result<Option<search::ai::Analysis>, String> {
    state.validate().map_err(str::to_owned)?;
    if retained
        .as_ref()
        .is_none_or(|s| s.id != spec.session_id || s.state != *state)
    {
        // Limit imported oracle values to this request. The UI sends at most one
        // root and its legal children; ownership/inventory remain in each key.
        let mut requested = vec![*state];
        for m in state.legal_moves() {
            let mut child = *state;
            child.apply_move(m).map_err(str::to_owned)?;
            requested.push(child);
        }
        let mut known = EvaluationMap::new();
        for child in &requested {
            if let Some(value) = search::lookup_exact(&spec.known, child) {
                known.insert(child.position_key(), value);
            }
        }
        if state.single_colors().is_some() {
            KNOWN.with(|cached| {
                for child in &requested {
                    if let Some(value) = search::lookup_exact(&cached.borrow(), child) {
                        known.insert(child.position_key(), value);
                    }
                }
            });
        }
        let full = state
            .cage
            .grid
            .iter()
            .flatten()
            .flatten()
            .filter(|c| c.is_some())
            .count()
            == 24;
        let missing = requested
            .iter()
            .any(|s| search::lookup_exact(&known, s).is_none());
        let oracle = if full && missing && state.outcome().is_none() {
            Some(search::general::Search::new(state)?)
        } else {
            None
        };
        *retained = Some(AiSession {
            id: spec.session_id,
            state: *state,
            known,
            requested,
            oracle,
            search: None,
        });
    }
    let session = retained.as_mut().unwrap();
    let mut incoming = EvaluationMap::new();
    for child in &session.requested {
        if let Some(value) = search::lookup_exact(&spec.known, child) {
            incoming.insert(child.position_key(), value);
        }
    }
    let changed = incoming
        .iter()
        .any(|(key, value)| session.known.get(key) != Some(value));
    search::merge_exact(&mut session.known, incoming)?;
    if changed {
        // A concurrently completed proof outranks older approximate work.
        session.search = None;
    }
    if let Some(oracle) = session.oracle.as_mut() {
        // The complete full-board graph is small and independent of heuristic
        // search. It yields between batches just like ordinary exact evaluation.
        let result = oracle.run(
            state,
            search::general::Budget {
                max_steps: 4000,
                max_records: 0,
                max_bytes: 16 * 1024 * 1024,
                max_horizon: 0,
            },
        )?;
        search::merge_exact(&mut session.known, result.values)?;
        if !result.complete && !result.at_capacity && !result.exhausted {
            return Ok(None);
        }
        session.oracle = None;
    }
    if session.search.is_none() {
        session.search = Some(search::ai::Search::new(
            state,
            &session.known,
            search::ai::Limits::default(),
        )?);
    }
    Ok(Some(session.search.as_mut().unwrap().run(
        search::ai::Budget {
            max_nodes: 4000,
            max_millis: spec.max_millis.clamp(1, 40),
        },
    )))
}

#[oneshot]
pub async fn EvaluationTask(spec: EvaluationTaskSpec) -> EvaluationTaskResult {
    if let Some(ai_spec) = spec.ai {
        let result =
            AI.with(|retained| evaluate_ai(&spec.state, ai_spec, &mut retained.borrow_mut()));
        let (ai, status) = match result {
            Ok(analysis) => {
                let status = if analysis.as_ref().is_some_and(|a| a.finished) {
                    TaskStatus::Complete
                } else {
                    TaskStatus::Continuing
                };
                (analysis, status)
            }
            Err(error) => (
                None,
                TaskStatus::Failed(format!("AI analysis could not be completed: {error}")),
            ),
        };
        return EvaluationTaskResult {
            request_id: spec.request_id,
            values: EvaluationMap::new(),
            status,
            ai,
        };
    }
    if spec.state.single_colors().is_none() {
        let result = GENERAL
            .with(|retained| evaluate_general(&spec.state, &mut retained.borrow_mut(), spec.batch));
        return match result {
            Ok((values, status)) => EvaluationTaskResult {
                request_id: spec.request_id,
                values,
                status,
                ai: None,
            },
            Err(error) => EvaluationTaskResult {
                request_id: spec.request_id,
                values: EvaluationMap::new(),
                status: TaskStatus::Failed(error),
                ai: None,
            },
        };
    }
    let (result, at_capacity) = KNOWN.with(|known| {
        SEARCH.with(|search| {
            let mut known = known.borrow_mut();
            let mut search = search.borrow_mut();
            let result = evaluate(&spec.state, &mut known, &mut search, spec.batch);
            let at_capacity = search
                .as_ref()
                .is_some_and(|s| s.proof.bounds.len() >= s.budget.max_positions);
            if known.len() > 500_000 {
                *known = super::evaluation::precomputed();
            }
            (result, at_capacity)
        })
    });
    let (values, status) = match result {
        Ok((values, complete)) => (
            values,
            status_after_batch(complete, spec.batch, at_capacity),
        ),
        Err(error) => (
            EvaluationMap::new(),
            TaskStatus::Failed(format!("Evaluation could not be completed: {error}")),
        ),
    };
    EvaluationTaskResult {
        request_id: spec.request_id,
        values,
        status,
        ai: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_worker_uses_complete_endgame_oracle_and_replaces_stale_sessions() {
        let state = crate::core::snapshot::decode(include_bytes!(
            "../../tests/fixtures/multicolor-exact/full-board-loss-in-6.rcg"
        ))
        .unwrap();
        let mut retained = None;
        let mut answer = None;
        for _ in 0..32 {
            if let Some(analysis) = evaluate_ai(
                &state,
                AiTaskSpec {
                    session_id: 1,
                    max_millis: 40,
                    known: EvaluationMap::new(),
                },
                &mut retained,
            )
            .unwrap()
            {
                if analysis.tactical_complete && analysis.best_move.is_some() {
                    answer = Some(analysis);
                    break;
                }
            }
        }
        let answer =
            answer.expect("Full-board oracle and AI must finish within normal worker batches");
        assert_eq!(
            answer.exact,
            Some(Evaluation::Win {
                winner: 0,
                plies: 6
            })
        );
        let mut child = state;
        child.apply_move(answer.best_move.unwrap()).unwrap();
        let reference =
            retrograde::solve(&state, &EvaluationMap::new(), Limits::default()).unwrap();
        assert_eq!(
            reference.values[&child.position_key()],
            Evaluation::Win {
                winner: 0,
                plies: 5
            }
        );
        let other = GameState::multicolor();
        evaluate_ai(
            &other,
            AiTaskSpec {
                session_id: 2,
                max_millis: 1,
                known: EvaluationMap::new(),
            },
            &mut retained,
        )
        .unwrap();
        assert_eq!(retained.as_ref().unwrap().id, 2);
        assert_eq!(retained.as_ref().unwrap().state, other);
        assert!(retained.as_ref().unwrap().oracle.is_none());
    }

    #[test]
    fn ai_worker_refreshes_advice_when_an_exact_proof_arrives() {
        let state = crate::core::snapshot::decode(include_bytes!(
            "../../tests/fixtures/multicolor-exact/tactic-win-in-3.rcg"
        ))
        .unwrap();
        let mut retained = None;
        let initial = evaluate_ai(
            &state,
            AiTaskSpec {
                session_id: 7,
                max_millis: 1,
                known: EvaluationMap::new(),
            },
            &mut retained,
        )
        .unwrap()
        .unwrap();
        assert_eq!(initial.exact, None);
        let mut proof_search = search::general::Search::new(&state).unwrap();
        let mut proved = EvaluationMap::new();
        for _ in 0..32 {
            let result = proof_search
                .run(
                    &state,
                    search::general::Budget {
                        max_steps: 20_000,
                        max_records: 100_000,
                        max_bytes: 32 * 1024 * 1024,
                        max_horizon: 5,
                    },
                )
                .unwrap();
            proved.extend(result.values);
            if proved.contains_key(&state.position_key()) {
                break;
            }
        }
        proof_search.proof.verify().unwrap();
        let expected = Some(Evaluation::Win {
            winner: 0,
            plies: 3,
        });
        assert_eq!(proved.get(&state.position_key()).copied(), expected);
        let refreshed = evaluate_ai(
            &state,
            AiTaskSpec {
                session_id: 7,
                max_millis: 1,
                known: proved,
            },
            &mut retained,
        )
        .unwrap()
        .unwrap();
        assert_eq!(refreshed.exact, expected);
    }

    #[test]
    fn ai_worker_uses_legacy_exact_values_without_changing_them() {
        let state = GameState::new(12, 12);
        let before = KNOWN.with(|known| known.borrow().clone());
        let mut retained = None;
        let mut answer = None;
        for _ in 0..32 {
            if let Some(analysis) = evaluate_ai(
                &state,
                AiTaskSpec {
                    session_id: 1,
                    max_millis: 40,
                    known: EvaluationMap::new(),
                },
                &mut retained,
            )
            .unwrap()
            {
                if analysis.tactical_complete {
                    answer = Some(analysis);
                    break;
                }
            }
        }
        let answer = answer.unwrap();
        let mut child = state;
        child.apply_move(answer.best_move.unwrap()).unwrap();
        assert_eq!(
            search::lookup_exact(&before, &child),
            Some(Evaluation::Win {
                winner: 0,
                plies: 10
            })
        );
        KNOWN.with(|known| assert_eq!(*known.borrow(), before));
    }
    #[test]
    fn general_worker_returns_material_draws_tactics_and_matching_namespaces() {
        let mut state = GameState::multicolor();
        state.remaining_cubies = [2; 6];
        let mut retained = None;
        let (values, status) = evaluate_general(&state, &mut retained, 0).unwrap();
        assert_eq!(status, TaskStatus::Complete);
        assert!(values.values().all(|v| *v == Evaluation::Draw));
        state.remaining_cubies = [1, 0, 0, 0, 0, 0];
        for x in 0..2 {
            state
                .cage
                .drop(crate::core::cubie::Cubie::White, (x, 0))
                .unwrap();
        }
        let mut found = EvaluationMap::new();
        for batch in 0..batch_limits(&state).0 {
            let (values, status) = evaluate_general(&state, &mut retained, batch).unwrap();
            found.extend(values);
            if status == TaskStatus::Complete {
                break;
            }
        }
        assert_eq!(
            found.get(&state.position_key()),
            Some(&Evaluation::Win {
                winner: 0,
                plies: 1
            })
        );
        let search = retained.as_ref().unwrap();
        assert!(search.matches(&state));
        search.proof.verify().unwrap();
        // Swapping owners changes the namespace even with identical stocks.
        state
            .color_owners
            .iter_mut()
            .flatten()
            .for_each(|owner| *owner ^= 1);
        evaluate_general(&state, &mut retained, 0).unwrap();
        assert!(retained.as_ref().unwrap().matches(&state));
        for batch in [0, 31, 32, 127] {
            let status = status_with_limits(false, batch, false, 32, 128);
            assert_eq!(
                matches!(status, TaskStatus::Continuing),
                batch == 0 || batch == 32
            );
        }
    }

    #[test]
    fn worker_publishes_exact_partial_results_and_reuses_only_matching_spaces() {
        let root = GameState::new(12, 12);
        let mut known = EvaluationMap::new();
        let mut retained = None;
        let (values, _) = evaluate(&root, &mut known, &mut retained, 0).unwrap();
        assert_eq!(
            values[&root.position_key()],
            Evaluation::Win {
                winner: 0,
                plies: 11
            }
        );
        retained.as_ref().unwrap().proof.verify().unwrap();
        let other = GameState::new(4, 2);
        evaluate(&other, &mut known, &mut retained, 0).unwrap();
        assert_eq!(
            retained.as_ref().unwrap().proof.space,
            Space::new(&other).unwrap()
        );
        let (_, complete) = evaluate(&GameState::new(3, 2), &mut known, &mut retained, 0).unwrap();
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
            let (values, complete) = evaluate(&state, &mut known, &mut retained, 0).unwrap();
            assert!(complete, "Unsolved small custom game {m},{n}");
            for (key, value) in values {
                if let Some(expected) = reference.values.get(&key) {
                    assert_eq!(*expected, value);
                } else {
                    assert_eq!(
                        reference.values[&key.swapped_players()].swapped_players(),
                        value
                    );
                }
            }
            assert_eq!(
                known[&state.position_key()],
                reference.values[&state.position_key()]
            );
        }
    }
    #[test]
    fn three_each_opening_uses_player_swaps_to_close_pass_cycles() {
        let root = GameState::new(3, 3);
        let mut known = EvaluationMap::new();
        let mut retained = None;
        let mut completed = false;
        for batch in 0..AUTOMATIC_BATCHES {
            let (_, complete) = evaluate(&root, &mut known, &mut retained, batch).unwrap();
            if complete {
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "3,3 opening incomplete after automatic continuations"
        );
        assert_eq!(known[&root.position_key()], Evaluation::Draw);
        for m in root.legal_moves() {
            let mut child = root;
            child.apply_move(m).unwrap();
            assert_eq!(
                known.get(&child.position_key()),
                Some(&Evaluation::Draw),
                "{m}"
            );
        }
        retained.unwrap().proof.verify().unwrap();
    }
    #[test]
    fn continuation_stops_at_explicit_limits_and_can_be_resumed() {
        assert_eq!(status_after_batch(false, 0, false), TaskStatus::Continuing);
        assert!(matches!(
            status_after_batch(false, AUTOMATIC_BATCHES - 1, false),
            TaskStatus::Paused {
                can_resume: true,
                ..
            }
        ));
        assert!(matches!(
            status_after_batch(false, 0, true),
            TaskStatus::Paused {
                can_resume: true,
                ..
            }
        ));
        assert_eq!(
            status_after_batch(false, AUTOMATIC_BATCHES, false),
            TaskStatus::Continuing
        );
        assert!(matches!(
            status_after_batch(false, MAX_BATCHES - 1, true),
            TaskStatus::Paused {
                can_resume: false,
                ..
            }
        ));
        assert_eq!(
            status_after_batch(true, MAX_BATCHES - 1, true),
            TaskStatus::Complete
        );
    }
    #[test]
    fn paused_search_resumes_without_discarding_completed_bounds() {
        let root = GameState::new(5, 4);
        let mut known = EvaluationMap::new();
        let mut retained = None;
        for batch in 0..AUTOMATIC_BATCHES {
            let (_, complete) = evaluate(&root, &mut known, &mut retained, batch).unwrap();
            let search = retained.as_ref().unwrap();
            let status = status_after_batch(
                complete,
                batch,
                search.proof.bounds.len() >= search.budget.max_positions,
            );
            if batch + 1 < AUTOMATIC_BATCHES {
                assert_eq!(status, TaskStatus::Continuing);
            } else {
                assert!(matches!(
                    status,
                    TaskStatus::Paused {
                        can_resume: true,
                        ..
                    }
                ));
            }
        }
        let search = retained.as_ref().unwrap();
        let previous_keys: Vec<_> = search.proof.bounds.keys().copied().collect();
        let previous_calls = search.calls;
        evaluate(&root, &mut known, &mut retained, AUTOMATIC_BATCHES).unwrap();
        let search = retained.as_ref().unwrap();
        assert!(search.calls > previous_calls);
        assert_eq!(search.budget.max_positions, 500_000);
        assert!(
            previous_keys
                .iter()
                .all(|k| search.proof.bounds.contains_key(k))
        );
        search.proof.verify().unwrap();
    }
}
