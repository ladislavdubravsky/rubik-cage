//! Reproducible, bounded exact-search experiments; no heuristic values are reported.
//! Uses the current simulator continuation rules, including play after the last
//! drop. Physical rules that end on that drop do not have these full-board endgames.
//! cargo run --release --example explore_multicolor -- --seconds 10 --discover-seconds 15
//! Optional --csv PATH writes measurements; --fixtures DIR writes importable snapshots.
//! --full-board runs complete full-board graphs; --position PATH compares a saved
//! position against a complete graph (intended for small closed endgame graphs).
use rubik_cage::{
    core::{cubie::Cubie, game::GameState, r#move::Move, snapshot},
    search::{
        Evaluation, EvaluationMap,
        general::{Budget, Search},
        retrograde,
    },
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Fixture {
    name: String,
    state: GameState,
    moves: Vec<Move>,
}

fn opening(stock: u8) -> GameState {
    GameState::with_colors(GameState::multicolor().color_owners, [stock; 6]).unwrap()
}
fn budget(horizon: u16) -> Budget {
    Budget {
        max_steps: 100_000,
        max_records: 1_000_000,
        max_bytes: 256 * 1024 * 1024,
        max_horizon: horizon,
    }
}
fn next(seed: &mut u64) -> usize {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as usize
}
fn random_fixture(seed: &mut u64, plies: usize, index: usize) -> Fixture {
    let mut state = opening(4);
    let mut moves = Vec::new();
    for _ in 0..plies {
        let candidates: Vec<_> = state
            .legal_moves()
            .into_iter()
            .filter_map(|m| {
                let mut child = state;
                child.apply_move(m).unwrap();
                child.outcome().is_none().then_some((m, child))
            })
            .collect();
        if candidates.is_empty() {
            break;
        }
        let (m, child) = candidates[next(seed) % candidates.len()];
        moves.push(m);
        state = child;
    }
    Fixture {
        name: format!("sample-{index}"),
        state,
        moves,
    }
}
fn immediate_win(state: &GameState) -> bool {
    state.legal_moves().into_iter().any(|m| {
        let mut child = *state;
        child.apply_move(m).unwrap();
        Evaluation::terminal(&child).and_then(|v| v.winner()) == Some(state.player_to_move.id)
    })
}
fn discovery(seconds: f64) -> Vec<Fixture> {
    let started = Instant::now();
    let mut seed = 0x4c41_4745_2026_u64;
    let mut selected = Vec::new();
    let mut index = 0;
    while started.elapsed().as_secs_f64() < seconds && selected.len() < 3 {
        let plies = 10 + next(&mut seed) % 13;
        let fixture = random_fixture(&mut seed, plies, index);
        index += 1;
        if immediate_win(&fixture.state) {
            continue;
        }
        let mut search = Search::new(&fixture.state).unwrap();
        let began = Instant::now();
        while began.elapsed() < Duration::from_millis(160) {
            let result = search.run(&fixture.state, budget(5)).unwrap();
            if let Some(Evaluation::Win { winner, plies }) =
                result.values.get(&fixture.state.position_key())
            {
                if *plies >= 3 {
                    eprintln!(
                        "Discovered {}: player {} wins in {} after {} legal moves",
                        fixture.name,
                        winner + 1,
                        plies,
                        fixture.moves.len()
                    );
                    selected.push(fixture);
                }
                break;
            }
            if result.complete || result.exhausted || result.at_capacity {
                break;
            }
        }
    }
    eprintln!(
        "Discovery sampled {index} positions in {:.3}s; retained {} nontrivial fixtures",
        started.elapsed().as_secs_f64(),
        selected.len()
    );
    selected
}
fn full_boards(dir: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "Full-board results use simulator continuation rules, not the physical last-drop draw rule."
    );
    if let Some(dir) = &dir {
        std::fs::create_dir_all(dir)?;
    }
    let mut seed = 0x4444_2026_u64;
    let mut found = 0;
    for attempt in 0..1000 {
        let mut state = opening(4);
        let mut moves = Vec::new();
        for _ in 0..24 {
            let candidates: Vec<_> = state
                .legal_moves()
                .into_iter()
                .filter(|m| matches!(m, Move::Drop { .. }))
                .filter_map(|m| {
                    let mut child = state;
                    child.apply_move(m).unwrap();
                    child.outcome().is_none().then_some((m, child))
                })
                .collect();
            if candidates.is_empty() {
                break;
            }
            let (m, child) = candidates[next(&mut seed) % candidates.len()];
            moves.push(m);
            state = child;
        }
        if moves.len() != 24 {
            continue;
        }
        let started = Instant::now();
        let solved = retrograde::solve(
            &state,
            &EvaluationMap::new(),
            retrograde::Limits {
                max_states: 100_000,
                max_edges: 1_000_000,
            },
        )?;
        let solve_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        retrograde::verify(&solved.values)?;
        let verify_ms = started.elapsed().as_secs_f64() * 1000.0;
        let value = solved.values[&state.position_key()];
        println!(
            "full-444-{found}: attempt={attempt}, value={value:?}, stats={:?}, solve_ms={solve_ms:.3}, verify_ms={verify_ms:.3}",
            solved.stats
        );
        println!("path={moves:?}");
        for m in state.legal_moves() {
            let mut child = state;
            child.apply_move(m)?;
            let value = solved.values[&child.position_key()];
            println!("  {m:?}: {value:?}");
            if let Some(dir) = &dir {
                if value
                    == (Evaluation::Win {
                        winner: 0,
                        plies: 6,
                    })
                {
                    let mut history = moves.clone();
                    history.push(m);
                    std::fs::write(
                        dir.join("full-board-loss-in-6.rcg"),
                        snapshot::encode(&child)?,
                    )?;
                    std::fs::write(
                        dir.join("full-board-loss-in-6.txt"),
                        format!("Starting inventory [4;6]\nMoves {history:?}\nResult {value:?}\n"),
                    )?;
                }
            }
        }
        if let Some(dir) = &dir {
            std::fs::write(
                dir.join(format!("full-444-{found}.rcg")),
                snapshot::encode(&state)?,
            )?;
            std::fs::write(
                dir.join(format!("full-444-{found}.txt")),
                format!("Starting inventory [4;6]\nMoves {moves:?}\nResult {value:?}\n"),
            )?;
        }
        found += 1;
        if found == 5 {
            break;
        }
    }
    assert!(found > 0, "No legal full-board fixture found");
    Ok(())
}
fn inspect_position(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let state = snapshot::decode(&std::fs::read(path)?)?;
    let started = Instant::now();
    let solved = retrograde::solve(&state, &EvaluationMap::new(), retrograde::Limits::default())?;
    let solve_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    retrograde::verify(&solved.values)?;
    let verify_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{path}: {:?}, {:?}, solve_ms={solve_ms:.3}, verify_ms={verify_ms:.3}",
        solved.values[&state.position_key()],
        solved.stats
    );
    let started = Instant::now();
    let mut search = Search::new(&state)?;
    for batches in 1..=1000 {
        let result = search.run(
            &state,
            Budget {
                max_steps: 4000,
                max_records: 25_000,
                max_bytes: 16 * 1024 * 1024,
                max_horizon: 6,
            },
        )?;
        for (key, value) in &result.values {
            assert_eq!(solved.values.get(key), Some(value));
        }
        if result.complete {
            let search_ms = started.elapsed().as_secs_f64() * 1000.0;
            search.proof.verify()?;
            println!(
                "general scheduler: {batches} batches, {search_ms:.3}ms, {} steps, {} records, {} estimated bytes; all requested values agree with complete graph",
                search.steps,
                search.proof.len(),
                search.estimated_bytes()
            );
            return Ok(());
        }
        if result.at_capacity || result.exhausted || started.elapsed() > Duration::from_secs(5) {
            break;
        }
    }
    Err("General scheduler did not complete the fixture within budget".into())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|x| x == name)
            .and_then(|i| args.get(i + 1))
    };
    let seconds = arg("--seconds")
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(10.0);
    let discover_seconds = arg("--discover-seconds")
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(15.0);
    if let Some(path) = arg("--position") {
        return inspect_position(path);
    }
    let fixture_dir = arg("--fixtures").map(PathBuf::from);
    if args.iter().any(|x| x == "--full-board") {
        return full_boards(fixture_dir);
    }
    if let Some(dir) = &fixture_dir {
        std::fs::create_dir_all(dir)?;
    }
    let mut fixtures = vec![
        Fixture {
            name: "opening-333-333".into(),
            state: opening(3),
            moves: vec![],
        },
        Fixture {
            name: "opening-444-444".into(),
            state: opening(4),
            moves: vec![],
        },
    ];
    let mut middle = Fixture {
        name: "midgame-444-444".into(),
        state: opening(4),
        moves: vec![],
    };
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
        let m = Move::Drop { color, column };
        middle.state.apply_move(m)?;
        middle.moves.push(m);
    }
    fixtures.push(middle);
    fixtures.extend(discovery(discover_seconds));
    let mut csv = "fixture,search_ms,verify_ms,steps,expansions,calls,cache_hits,records,estimated_bytes,max_horizon,solved_moves,legal_moves,root,stop\n".to_owned();
    for fixture in fixtures {
        assert!(fixture.state.outcome().is_none());
        fixture.state.validate()?;
        let mut search = Search::new(&fixture.state)?;
        let start = Instant::now();
        let mut values = EvaluationMap::new();
        let mut stop = "time";
        while start.elapsed().as_secs_f64() < seconds {
            let result = search.run(&fixture.state, budget(12))?;
            values.extend(result.values);
            if values.contains_key(&fixture.state.position_key()) {
                stop = "root-proved";
                break;
            }
            if result.complete {
                stop = "complete";
                break;
            }
            if result.at_capacity {
                stop = "capacity";
                break;
            }
            if result.exhausted {
                stop = "horizon";
                break;
            }
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let verify_start = Instant::now();
        // The verifier uses core game transitions, independently of compact search.
        let useful = !values.is_empty();
        if useful {
            search.proof.verify()?;
        }
        let verify_ms = if useful {
            verify_start.elapsed().as_secs_f64() * 1000.0
        } else {
            0.0
        };
        let legal = fixture.state.legal_moves();
        let solved = legal
            .iter()
            .filter(|&&m| {
                let mut child = fixture.state;
                child.apply_move(m).unwrap();
                Evaluation::terminal(&child).is_some() || values.contains_key(&child.position_key())
            })
            .count();
        let root = match values.get(&fixture.state.position_key()) {
            Some(Evaluation::Win { winner, plies }) => format!("P{}-win-{plies}", winner + 1),
            Some(Evaluation::Draw) => "Draw".into(),
            None => "Unknown".into(),
        };
        let row = format!(
            "{},{elapsed:.3},{verify_ms:.3},{},{},{},{},{},{},12,{solved},{},{root},{stop}\n",
            fixture.name,
            search.steps,
            search.proof.expansions.get(),
            search.proof.calls.get(),
            search.proof.hits.get(),
            search.proof.len(),
            search.estimated_bytes(),
            legal.len()
        );
        print!("{row}");
        csv.push_str(&row);
        eprintln!("{} path = {:?}", fixture.name, fixture.moves);
        for m in legal {
            let mut child = fixture.state;
            child.apply_move(m)?;
            if let Some(value) =
                Evaluation::terminal(&child).or_else(|| values.get(&child.position_key()).copied())
            {
                eprintln!("  {m:?}: {value:?}");
            }
        }
        if let Some(dir) = &fixture_dir {
            std::fs::write(
                dir.join(format!("{}.rcg", fixture.name)),
                snapshot::encode(&fixture.state)?,
            )?;
            std::fs::write(
                dir.join(format!("{}.txt", fixture.name)),
                format!(
                    "Starting inventory {:?}\nMoves {:?}\nResult {root}\n",
                    fixture.state.inventories(),
                    fixture.moves
                ),
            )?;
        }
    }
    if let Some(path) = arg("--csv") {
        std::fs::write(path, csv)?;
    }
    Ok(())
}
