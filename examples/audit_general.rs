//! Same reproducible release audit on native and wasm32-unknown-unknown.
//! Build with --features search-audit. Results are CSV on stdout (native) or
//! globalThis.auditResult (WASM). Audit code is excluded from normal app builds.
use rubik_cage::{
    core::{cubie::Cubie, game::GameState, r#move::Move},
    search::{Evaluation, EvaluationMap, general, general_reference},
};

#[cfg(not(target_arch = "wasm32"))]
fn now() -> f64 {
    static START: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    START.elapsed().as_secs_f64() * 1000.0
}
#[cfg(target_arch = "wasm32")]
fn now() -> f64 {
    web_sys::js_sys::Date::now()
}
fn fixtures() -> Vec<(&'static str, GameState)> {
    let opening = GameState::multicolor();
    let mut mid = opening;
    for (color, column) in [
        (Cubie::White, (0, 0)),
        (Cubie::Yellow, (2, 2)),
        (Cubie::Blue, (0, 2)),
        (Cubie::Red, (2, 0)),
        (Cubie::Green, (1, 0)),
        (Cubie::Orange, (1, 2)),
        (Cubie::White, (0, 1)),
        (Cubie::Yellow, (2, 1)),
    ] {
        mid.apply_move(Move::Drop { color, column }).unwrap();
    }
    let mut late = mid;
    loop {
        let next = late
            .legal_moves()
            .into_iter()
            .filter(|m| matches!(m, Move::Drop { .. }))
            .find_map(|m| {
                let mut next = late;
                next.apply_move(m).unwrap();
                (next.outcome().is_none()).then_some(next)
            });
        match next {
            Some(next) => late = next,
            None => break,
        }
    }
    let mut tactic = opening;
    for x in 0..2 {
        tactic.cage.drop(Cubie::Green, (x, 0)).unwrap();
        tactic.remaining_cubies[5] -= 1;
    }
    vec![
        ("opening", opening),
        ("midgame", mid),
        ("late", late),
        ("tactic", tactic),
        (
            "small",
            GameState::with_colors(opening.color_owners, [3, 0, 0, 0, 1, 0]).unwrap(),
        ),
    ]
}
fn main() {
    let mut report = String::from(
        "fixture,backend,trial,ms,steps,expansions,calls,cache_hits,records,estimated_bytes,batch_p50_ms,batch_p95_ms,batch_max_ms,solved_moves,legal_moves\n",
    );
    for (name, state) in fixtures() {
        let mut agreed = EvaluationMap::new();
        for backend in ["reference", "compact", "relabel", "ordered"] {
            for trial in 0..3 {
                let mut baseline = general_reference::Search::new(&state).unwrap();
                let mut optimized = general::Search::with_options(
                    &state,
                    backend != "compact",
                    backend == "ordered",
                )
                .unwrap();
                let start = now();
                let mut latencies = Vec::new();
                let mut values = EvaluationMap::new();
                for _ in 0..32 {
                    let before = now();
                    let (fresh, stop) = if backend == "reference" {
                        let result = baseline
                            .run(
                                &state,
                                general_reference::Budget {
                                    max_steps: 4000,
                                    max_records: 25_000,
                                    max_bytes: 16 * 1024 * 1024,
                                    max_horizon: 6,
                                },
                            )
                            .unwrap();
                        (
                            result.values,
                            result.complete || result.exhausted || result.at_capacity,
                        )
                    } else {
                        let result = optimized
                            .run(
                                &state,
                                general::Budget {
                                    max_steps: 4000,
                                    max_records: 25_000,
                                    max_bytes: 16 * 1024 * 1024,
                                    max_horizon: 6,
                                },
                            )
                            .unwrap();
                        (
                            result.values,
                            result.complete || result.exhausted || result.at_capacity,
                        )
                    };
                    latencies.push(now() - before);
                    values.extend(fresh);
                    if stop {
                        break;
                    }
                }
                let elapsed = now() - start;
                for (&key, &value) in &values {
                    if let Some(previous) = agreed.insert(key, value) {
                        assert_eq!(previous, value);
                    }
                }
                let moves = state.legal_moves();
                let solved = moves
                    .iter()
                    .filter(|&&m| {
                        let mut next = state;
                        next.apply_move(m).unwrap();
                        Evaluation::terminal(&next).is_some()
                            || values.contains_key(&next.position_key())
                    })
                    .count();
                let (steps, expansions, calls, hits, records, bytes) = if backend == "reference" {
                    (
                        baseline.steps,
                        baseline.proof.expansions.get(),
                        baseline.proof.calls.get(),
                        baseline.proof.hits.get(),
                        baseline.proof.len(),
                        baseline.estimated_bytes(),
                    )
                } else {
                    (
                        optimized.steps,
                        optimized.proof.expansions.get(),
                        optimized.proof.calls.get(),
                        optimized.proof.hits.get(),
                        optimized.proof.len(),
                        optimized.estimated_bytes(),
                    )
                };
                latencies.sort_by(f64::total_cmp);
                report += &format!(
                    "{name},{backend},{trial},{elapsed:.3},{steps},{expansions},{calls},{hits},{records},{bytes},{:.3},{:.3},{:.3},{solved},{}\n",
                    latencies[latencies.len() / 2],
                    latencies[(latencies.len() * 95 / 100).min(latencies.len() - 1)],
                    latencies.last().unwrap(),
                    moves.len()
                );
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    print!("{report}");
    #[cfg(target_arch = "wasm32")]
    web_sys::js_sys::Reflect::set(
        &web_sys::js_sys::global(),
        &"auditResult".into(),
        &report.into(),
    )
    .unwrap();
}
