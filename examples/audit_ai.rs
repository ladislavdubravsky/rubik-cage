//! Native AI latency and local-oracle audit, not a playing-strength tournament.
//! cargo run --release --example audit_ai -- --csv docs/benchmarks/ai-baseline.csv
//! Add --long for 10-second budgets everywhere, or --opening-long only for the
//! four-each opening. Full-board oracles use the simulator
//! continuation variant; the physical last-drop stopping rule has no such endgame.
//! The nodes column counts elementary work steps, including tactical probes and
//! returns. Random rows report exact expected choice quality, not sampled games.
use rubik_cage::{
    core::{
        cubie::Cubie,
        game::{GameState, Outcome},
        r#move::Move,
        snapshot,
    },
    search::{
        Evaluation, EvaluationMap,
        ai::{Analysis, Budget, Limits, Search},
        general, retrograde,
    },
};
use std::time::Instant;

fn after(state: &GameState, m: Move) -> GameState {
    let mut child = *state;
    child.apply_move(m).unwrap();
    child
}
fn immediate_win(state: &GameState, m: Move) -> bool {
    after(state, m).outcome() == Some(Outcome::Win(state.player_to_move.id))
}
fn safe(state: &GameState, m: Move) -> bool {
    let child = after(state, m);
    if let Some(outcome) = child.outcome() {
        return outcome != Outcome::Win(1 - state.player_to_move.id);
    }
    !child
        .legal_moves()
        .into_iter()
        .any(|reply| immediate_win(&child, reply))
}
fn rank(value: Evaluation, player: u8) -> (u8, i64) {
    match value {
        Evaluation::Win { winner, plies } if winner == player => (2, -i64::from(plies)),
        Evaluation::Draw => (1, 0),
        Evaluation::Win { plies, .. } => (0, i64::from(plies)),
    }
}
fn analyse(state: &GameState, known: &EvaluationMap, milliseconds: u32) -> (Analysis, f64) {
    let started = Instant::now();
    let mut search = Search::new(
        state,
        known,
        Limits {
            max_depth: 16,
            max_records: 100_000,
        },
    )
    .unwrap();
    let mut result = search.run(Budget {
        max_nodes: 0,
        max_millis: 0,
    });
    while started.elapsed().as_millis() < u128::from(milliseconds) && !result.finished {
        let remaining = milliseconds.saturating_sub(started.elapsed().as_millis() as u32);
        result = search.run(Budget {
            max_nodes: 100_000,
            max_millis: remaining.clamp(1, 20),
        });
    }
    assert!(
        result
            .best_move
            .is_none_or(|m| state.legal_moves().contains(&m))
    );
    (result, started.elapsed().as_secs_f64() * 1000.0)
}
fn csv_row(out: &mut String, mut values: Vec<String>) {
    let uses_ai = values[1].starts_with("alpha-beta");
    if !uses_ai {
        values[3].clear();
    } // Baseline selection latency was not timed.
    values.push("simulator-continue-after-last-drop".into());
    values.push(if uses_ai { "16".into() } else { String::new() });
    values.push(if uses_ai {
        "100000".into()
    } else {
        String::new()
    });
    out.push_str(
        &values
            .into_iter()
            .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
}
fn verify_tactical_choices(state: &GameState, choices: &[Move]) -> Result<EvaluationMap, String> {
    let mut wanted: Vec<_> = choices
        .iter()
        .map(|&m| after(state, m).position_key())
        .collect();
    wanted.push(state.position_key());
    let mut search = general::Search::new(state)?;
    let mut values = EvaluationMap::new();
    for _ in 0..100 {
        let result = search.run(
            state,
            general::Budget {
                max_steps: 20_000,
                max_records: 100_000,
                max_bytes: 32 * 1024 * 1024,
                max_horizon: 8,
            },
        )?;
        values.extend(result.values);
        if wanted.iter().all(|k| values.contains_key(k)) || result.at_capacity || result.exhausted {
            break;
        }
    }
    search.proof.verify()?;
    Ok(values)
}
fn fixtures() -> Vec<(&'static str, GameState)> {
    let opening = GameState::with_colors(GameState::multicolor().color_owners, [4; 6]).unwrap();
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
    let decode = |bytes| snapshot::decode(bytes).unwrap();
    vec![
        ("opening-444-444", opening),
        ("midgame-444-444", mid),
        (
            "tactic-win-in-3",
            decode(include_bytes!(
                "../tests/fixtures/multicolor-exact/tactic-win-in-3.rcg"
            )),
        ),
        (
            "tactic-win-in-5",
            decode(include_bytes!(
                "../tests/fixtures/multicolor-exact/tactic-win-in-5.rcg"
            )),
        ),
        (
            "tactic-loss-in-4",
            decode(include_bytes!(
                "../tests/fixtures/multicolor-exact/tactic-loss-in-4.rcg"
            )),
        ),
        (
            "full-board-draw",
            decode(include_bytes!(
                "../tests/fixtures/multicolor-exact/full-board-draw.rcg"
            )),
        ),
        (
            "full-board-loss-in-6",
            decode(include_bytes!(
                "../tests/fixtures/multicolor-exact/full-board-loss-in-6.rcg"
            )),
        ),
    ]
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let mut out = String::from(
        "fixture,policy,budget_ms,elapsed_ms,completed_depth,nodes,tactical_complete,finished,selected_move,exact,immediate_win_available,selected_immediate_win,safe_move_available,selected_safe_move,oracle_outcome_optimal,oracle_distance_optimal,rules,max_depth,max_records\n",
    );
    let mut budgets = vec![100, 1000];
    if args.iter().any(|a| a == "--long") {
        budgets.push(10_000);
    }
    for (name, state) in fixtures() {
        let moves = state.legal_moves();
        let win_available = moves.iter().any(|&m| immediate_win(&state, m));
        let safe_available = moves.iter().any(|&m| safe(&state, m));
        let mut fixture_budgets = budgets.clone();
        if name == "opening-444-444"
            && args.iter().any(|a| a == "--opening-long")
            && !fixture_budgets.contains(&10_000)
        {
            fixture_budgets.push(10_000);
        }
        let measured: Vec<_> = fixture_budgets
            .iter()
            .map(|&ms| {
                let (result, elapsed) = analyse(&state, &EvaluationMap::new(), ms);
                (ms, result, elapsed)
            })
            .collect();
        // These proofs run after timed analysis and are never supplied to the AI.
        let oracle = if name.starts_with("tactic-") {
            let choices: Vec<_> = measured
                .iter()
                .filter_map(|(_, a, _)| a.best_move)
                .collect();
            verify_tactical_choices(&state, &choices)?
        } else {
            EvaluationMap::new()
        };
        for (ms, result, elapsed) in measured {
            let chosen = result.best_move.unwrap();
            eprintln!(
                "{name} {ms}ms: depth {}, {} nodes, {elapsed:.2}ms, {chosen:?}, score {}, exact {:?}",
                result.completed_depth, result.nodes, result.score, result.exact
            );
            let verified = oracle
                .get(&state.position_key())
                .zip(oracle.get(&after(&state, chosen).position_key()));
            let (outcome, distance) =
                verified.map_or((String::new(), String::new()), |(root, child)| {
                    eprintln!("  independently verified: root {root:?}, selected child {child:?}");
                    let same_winner = root.winner() == child.winner();
                    let matching_distance = match (root.plies(), child.plies()) {
                        (Some(a), Some(b)) => a == b + 1,
                        (None, None) => true,
                        _ => false,
                    };
                    (
                        (if same_winner { "1" } else { "0" }).into(),
                        (if same_winner && matching_distance {
                            "1"
                        } else {
                            "0"
                        })
                        .into(),
                    )
                });
            csv_row(
                &mut out,
                vec![
                    name.into(),
                    "alpha-beta-no-oracle".into(),
                    ms.to_string(),
                    format!("{elapsed:.3}"),
                    result.completed_depth.to_string(),
                    result.nodes.to_string(),
                    result.tactical_complete.to_string(),
                    result.finished.to_string(),
                    format!("{chosen:?}"),
                    format!("{:?}", result.exact),
                    win_available.to_string(),
                    immediate_win(&state, chosen).to_string(),
                    safe_available.to_string(),
                    safe(&state, chosen).to_string(),
                    outcome,
                    distance,
                ],
            );
        }
    }
    let mut totals = [(0.0, 0.0, 0usize); 4];
    for (family, bytes) in [
        (
            "draw-orbit",
            include_bytes!("../tests/fixtures/multicolor-exact/full-board-draw.rcg").as_slice(),
        ),
        (
            "loss-orbit",
            include_bytes!("../tests/fixtures/multicolor-exact/full-board-loss-in-6.rcg")
                .as_slice(),
        ),
    ] {
        let initial = snapshot::decode(bytes)?;
        let oracle = retrograde::solve(&initial, &EvaluationMap::new(), Default::default())?.values;
        retrograde::verify(&oracle)?;
        let mut keys: Vec<_> = oracle
            .keys()
            .copied()
            .filter(|k| k.to_state().outcome().is_none())
            .collect();
        keys.sort();
        for sample in 0..12 {
            let state = keys[sample * keys.len() / 12].to_state();
            let moves = state.legal_moves();
            let ranks: Vec<_> = moves
                .iter()
                .map(|&m| {
                    rank(
                        oracle[&after(&state, m).position_key()],
                        state.player_to_move.id,
                    )
                })
                .collect();
            let optimal = *ranks.iter().max().unwrap();
            let greedy = moves
                .iter()
                .copied()
                .find(|&m| immediate_win(&state, m))
                .or_else(|| moves.iter().copied().find(|&m| safe(&state, m)))
                .unwrap_or(moves[0]);
            let (blind, blind_ms) = analyse(&state, &EvaluationMap::new(), 100);
            let (informed, informed_ms) = analyse(&state, &oracle, 100);
            for (policy, (name, choice, elapsed, analysis)) in [
                ("uniform-random-expectation", None, 0.0, None),
                ("tactical-greedy", Some(greedy), 0.0, None),
                (
                    "alpha-beta-no-oracle",
                    blind.best_move,
                    blind_ms,
                    Some(&blind),
                ),
                (
                    "alpha-beta-exact-oracle",
                    informed.best_move,
                    informed_ms,
                    Some(&informed),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let (outcome, distance) = if let Some(m) = choice {
                    let r = ranks[moves.iter().position(|&x| x == m).unwrap()];
                    (
                        if r.0 == optimal.0 { 1.0 } else { 0.0 },
                        if r == optimal { 1.0 } else { 0.0 },
                    )
                } else {
                    (
                        ranks.iter().filter(|r| r.0 == optimal.0).count() as f64
                            / ranks.len() as f64,
                        ranks.iter().filter(|r| **r == optimal).count() as f64 / ranks.len() as f64,
                    )
                };
                totals[policy].0 += outcome;
                totals[policy].1 += distance;
                totals[policy].2 += 1;
                csv_row(
                    &mut out,
                    vec![
                        format!("{family}-{sample}"),
                        name.into(),
                        if analysis.is_some() {
                            "100".into()
                        } else {
                            "0".into()
                        },
                        format!("{elapsed:.3}"),
                        analysis.map_or(String::new(), |a| a.completed_depth.to_string()),
                        analysis.map_or(String::new(), |a| a.nodes.to_string()),
                        analysis.map_or(String::new(), |a| a.tactical_complete.to_string()),
                        analysis.map_or(String::new(), |a| a.finished.to_string()),
                        format!("{choice:?}"),
                        analysis.map_or(String::new(), |a| format!("{:?}", a.exact)),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        format!("{outcome:.6}"),
                        format!("{distance:.6}"),
                    ],
                );
            }
        }
    }
    for (name, (outcome, distance, n)) in [
        "uniform-random expectation",
        "tactical-greedy",
        "alpha-beta without oracle",
        "alpha-beta with oracle",
    ]
    .into_iter()
    .zip(totals)
    {
        eprintln!(
            "{name}: oracle-optimal outcome {outcome:.3}/{n}, distance {distance:.3}/{n}; local fixtures only, not game win rates"
        );
    }
    if let Some(path) = args
        .iter()
        .position(|a| a == "--csv")
        .and_then(|i| args.get(i + 1))
    {
        std::fs::write(path, out)?;
    } else {
        print!("{out}");
    }
    Ok(())
}
