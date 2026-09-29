//! Independent core-rule checks for approximate move selection.
use rubik_cage::{
    core::{
        cage::Cage,
        cubie::Cubie,
        game::{GameState, Outcome},
        r#move::{Layer, Move, Rotation},
        snapshot,
    },
    search::{
        Evaluation, EvaluationMap,
        ai::{self, Analysis, Budget, Limits, Search},
        retrograde,
    },
};

fn opening() -> GameState {
    GameState::with_colors(GameState::multicolor().color_owners, [4; 6]).unwrap()
}
// Uppercase color + x,y; d/e/u +/- use the core clockwise/counterclockwise enum.
fn history(text: &str) -> GameState {
    let mut state = opening();
    for token in text.split_whitespace() {
        let bytes = token.as_bytes();
        let m = if token == "f" {
            Move::Flip
        } else if bytes[0].is_ascii_lowercase() {
            Move::RotateLayer {
                layer: match bytes[0] {
                    b'd' => Layer::Down,
                    b'e' => Layer::Equator,
                    b'u' => Layer::Up,
                    _ => panic!("bad layer"),
                },
                rotation: if bytes[1] == b'+' {
                    Rotation::Clockwise
                } else {
                    Rotation::CounterClockwise
                },
            }
        } else {
            Move::Drop {
                color: Cubie::from_char(bytes[0] as char).unwrap(),
                column: ((bytes[1] - b'0') as usize, (bytes[2] - b'0') as usize),
            }
        };
        state.apply_move(m).unwrap();
        assert!(state.outcome().is_none(), "Fixture must remain nonterminal");
    }
    state.validate().unwrap();
    state
}
fn flip_win() -> GameState {
    history("W01 R20 B00 d- B01 d+ B01 e+ B00 d-")
}
fn rotation_win() -> GameState {
    history("B20 R02 B02 O00 W21 R20 G10 O02 B20 e- f O20 B00 u- W20 R10 W01 O10 W12 Y00 f e+")
}
fn flip_defense() -> GameState {
    history("W00 R12 d- R12 u- Y00 B20 Y20 W12 Y10 W02 Y22 B10 O01 G02")
}
fn rotation_defense() -> GameState {
    history("B10 R20 d+ R12 G21 e+ G02 R01 B01 R00 G02")
}
fn after(state: &GameState, m: Move) -> GameState {
    let mut child = *state;
    child.apply_move(m).unwrap();
    child
}
fn wins(state: &GameState) -> Vec<Move> {
    state
        .legal_moves()
        .into_iter()
        .filter(|&m| after(state, m).outcome() == Some(Outcome::Win(state.player_to_move.id)))
        .collect()
}
fn safe(state: &GameState, m: Move) -> bool {
    let child = after(state, m);
    if let Some(outcome) = child.outcome() {
        return outcome != Outcome::Win(1 - state.player_to_move.id);
    }
    wins(&child).is_empty()
}
fn assert_legal_analysis(state: &GameState, value: &Analysis) {
    assert_eq!(value.position, state.position_key());
    if let Some(m) = value.best_move {
        assert!(state.legal_moves().contains(&m));
    }
    let mut next = *state;
    for &m in &value.principal_variation {
        next.apply_move(m)
            .expect("PV must use original orientation and legal exact colors");
    }
    if let Some(&m) = value.principal_variation.first() {
        assert_eq!(value.best_move, Some(m));
    }
}
fn analyse(state: &GameState, known: &EvaluationMap, depth: u16, tactics_only: bool) -> Analysis {
    let mut search = Search::new(
        state,
        known,
        Limits {
            max_depth: depth,
            max_records: 20_000,
        },
    )
    .unwrap();
    for _ in 0..1000 {
        let result = search.run(Budget {
            max_nodes: 20_000,
            max_millis: 40,
        });
        assert_legal_analysis(state, &result);
        if result.finished || (tactics_only && result.tactical_complete) {
            return result;
        }
    }
    panic!("Small validation search did not finish");
}

#[test]
fn immediate_wins_include_drops_rotations_and_gravity_after_flips() {
    let drop_win = history("G00 R22 G10 R21");
    for (state, class) in [(drop_win, 0), (rotation_win(), 1), (flip_win(), 2)] {
        let expected = wins(&state);
        assert!(!expected.is_empty());
        assert!(expected.iter().all(|m| match class {
            0 => matches!(m, Move::Drop { .. }),
            1 => matches!(m, Move::RotateLayer { .. }),
            _ => matches!(m, Move::Flip),
        }));
        let result = analyse(&state, &EvaluationMap::new(), 2, true);
        assert!(expected.contains(&result.best_move.unwrap()));
        assert!(result.tactical_complete);
    }
}

#[test]
fn tactical_scan_finds_the_only_safe_manipulations() {
    for (state, only_flip) in [(flip_defense(), true), (rotation_defense(), false)] {
        assert!(wins(&state).is_empty());
        let legal = state.legal_moves();
        let escapes: Vec<_> = legal.iter().copied().filter(|&m| safe(&state, m)).collect();
        assert!(!escapes.is_empty() && escapes.len() < legal.len());
        assert!(escapes.iter().all(|m| if only_flip {
            matches!(m, Move::Flip)
        } else {
            matches!(m, Move::RotateLayer { .. })
        }));
        let result = analyse(&state, &EvaluationMap::new(), 2, true);
        assert!(result.tactical_complete);
        assert!(
            escapes.contains(&result.best_move.unwrap()),
            "An unsafe reply survived a completed tactical scan"
        );
    }
}

#[test]
fn simultaneous_opposing_lines_are_a_terminal_draw() {
    let state = history(
        "G10 O01 W21 Y12 d- O00 G01 Y02 G12 Y21 G12 R02 W22 u- B10 Y01 B10 u- d- O21 d- u- d-",
    );
    let drawn = state
        .legal_moves()
        .into_iter()
        .map(|m| after(&state, m))
        .find(|s| s.outcome() == Some(Outcome::Draw))
        .unwrap();
    let owners: std::collections::HashSet<_> = drawn
        .cage
        .lines()
        .map(|(c, _)| drawn.owner(c).unwrap())
        .collect();
    assert_eq!(owners.len(), 2);
    let result = analyse(&drawn, &EvaluationMap::new(), 2, false);
    assert_eq!(result.best_move, None);
    assert_eq!(result.exact, Some(Evaluation::Draw));
    assert_eq!(result.score, 0);
}

fn oracle_rank(value: Evaluation, player: u8) -> (u8, i64) {
    match value {
        Evaluation::Win { winner, plies } if winner == player => (2, -i64::from(plies)),
        Evaluation::Draw => (1, 0),
        Evaluation::Win { plies, .. } => (0, i64::from(plies)),
    }
}
#[test]
fn exact_oracles_choose_shortest_wins_draws_and_longest_losses_without_mutation() {
    for bytes in [
        include_bytes!("fixtures/multicolor-exact/full-board-draw.rcg").as_slice(),
        include_bytes!("fixtures/multicolor-exact/full-board-loss-in-6.rcg").as_slice(),
    ] {
        let initial = snapshot::decode(bytes).unwrap();
        let oracle = retrograde::solve(&initial, &EvaluationMap::new(), Default::default())
            .unwrap()
            .values;
        let before = oracle.clone();
        let mut states = vec![initial];
        let winning = oracle
            .iter()
            .filter(|(k, v)| {
                k.to_state().outcome().is_none()
                    && matches!(v, Evaluation::Win { winner, .. } if *winner == k.turn())
            })
            .map(|(&k, _)| k)
            .min();
        if let Some(key) = winning {
            states.push(key.to_state());
        }
        for state in states {
            let root = oracle[&state.position_key()];
            let expected = state
                .legal_moves()
                .into_iter()
                .map(|m| {
                    oracle_rank(
                        oracle[&after(&state, m).position_key()],
                        state.player_to_move.id,
                    )
                })
                .max()
                .unwrap();
            let result = analyse(&state, &oracle, 3, false);
            let chosen = after(&state, result.best_move.unwrap());
            assert_eq!(
                oracle_rank(oracle[&chosen.position_key()], state.player_to_move.id),
                expected
            );
            assert_eq!(result.exact, Some(root));
            // A single certified optimal child must also beat optimistic
            // heuristic values for children absent from a partial oracle.
            let witness = state
                .legal_moves()
                .into_iter()
                .find_map(|m| {
                    let child = after(&state, m);
                    let value = oracle[&child.position_key()];
                    (oracle_rank(value, state.player_to_move.id) == expected)
                        .then_some((child.position_key(), value))
                })
                .unwrap();
            let partial = EvaluationMap::from([(state.position_key(), root), witness]);
            let result = analyse(&state, &partial, 3, false);
            assert_eq!(
                result.nodes, 0,
                "A certified optimal action needs no heuristic search"
            );
            assert!(result.finished);
            let chosen = after(&state, result.best_move.unwrap());
            assert_eq!(
                oracle_rank(oracle[&chosen.position_key()], state.player_to_move.id),
                expected
            );
        }
        assert_eq!(
            oracle, before,
            "Approximate search must not modify the exact cache"
        );
    }
}

fn transform(mut state: GameState, symmetry: usize, relabel: bool) -> GameState {
    let mut cage = Cage::new();
    for x in 0..3 {
        for y in 0..3 {
            let (mut a, mut b) = (if symmetry >= 4 { 2 - x } else { x }, y);
            for _ in 0..symmetry % 4 {
                (a, b) = (b, 2 - a);
            }
            cage.grid[a][b] = state.cage.grid[x][y];
        }
    }
    state.cage = cage;
    state.last_move = match state.last_move {
        Some(Move::RotateLayer { layer, rotation }) if symmetry >= 4 => Some(Move::RotateLayer {
            layer,
            rotation: if rotation == Rotation::Clockwise {
                Rotation::CounterClockwise
            } else {
                Rotation::Clockwise
            },
        }),
        Some(Move::Drop { .. }) => None,
        other => other,
    };
    if relabel {
        let map = |color| match color {
            Cubie::White => Cubie::Blue,
            Cubie::Blue => Cubie::White,
            Cubie::Yellow => Cubie::Red,
            Cubie::Red => Cubie::Yellow,
            other => other,
        };
        for color in state.cage.grid.iter_mut().flatten().flatten().flatten() {
            *color = map(*color);
        }
        state
            .remaining_cubies
            .swap(Cubie::White as usize, Cubie::Blue as usize);
        state
            .remaining_cubies
            .swap(Cubie::Yellow as usize, Cubie::Red as usize);
    }
    state = snapshot::decode(&snapshot::encode(&state).unwrap()).unwrap();
    state.validate().unwrap();
    state
}
#[test]
fn symmetries_and_color_relabeling_keep_original_move_coordinates_and_scores() {
    let state = rotation_win();
    let score = ai::heuristic(&state);
    for symmetry in 0..8 {
        for relabel in [false, true] {
            let transformed = transform(state, symmetry, relabel);
            assert_eq!(ai::heuristic(&transformed), score);
            let result = analyse(&transformed, &EvaluationMap::new(), 2, true);
            assert!(wins(&transformed).contains(&result.best_move.unwrap()));
        }
    }
    let mut other_turn = state;
    other_turn.player_to_move = state.players[1 - state.player_to_move.id as usize];
    other_turn.last_move = None;
    assert_eq!(ai::heuristic(&other_turn), -score);
}

fn minimax(state: &GameState, depth: u16, ply: i32) -> i32 {
    if let Some(value) = Evaluation::terminal(state) {
        return match value {
            Evaluation::Draw => 0,
            Evaluation::Win { winner, .. } if winner == state.player_to_move.id => {
                ai::MATE_SCORE - ply
            }
            Evaluation::Win { .. } => -ai::MATE_SCORE + ply,
        };
    }
    if depth == 0 {
        return ai::heuristic(state);
    }
    state
        .legal_moves()
        .into_iter()
        .map(|m| -minimax(&after(state, m), depth - 1, ply + 1))
        .max()
        .unwrap()
}
#[test]
fn shallow_search_matches_independent_core_minimax_on_quiet_openings() {
    // At depth <=2 there is at most one cubie per player: no pair can trigger
    // the engine's tactical leaf extension, so the leaf definitions coincide.
    for state in [opening(), GameState::new(4, 4)] {
        for depth in 1..=2 {
            let result = analyse(&state, &EvaluationMap::new(), depth, false);
            assert_eq!(result.completed_depth, depth);
            assert_eq!(result.score, minimax(&state, depth, 0));
            assert_eq!(
                result.score,
                -minimax(&after(&state, result.best_move.unwrap()), depth - 1, 1)
            );
            assert_eq!(result.exact, None);
        }
    }
}

#[test]
fn interruption_keeps_a_legal_fallback_and_completed_iterations() {
    let state = opening();
    let mut search = Search::new(
        &state,
        &EvaluationMap::new(),
        Limits {
            max_depth: 4,
            max_records: 2000,
        },
    )
    .unwrap();
    let initial = search.run(Budget {
        max_nodes: 0,
        max_millis: 0,
    });
    assert_legal_analysis(&state, &initial);
    assert!(initial.best_move.is_some());
    let mut last_depth = 0;
    let mut completed = None;
    for _ in 0..2000 {
        let result = search.run(Budget {
            max_nodes: 3,
            max_millis: 20,
        });
        assert_legal_analysis(&state, &result);
        assert!(result.completed_depth >= last_depth);
        last_depth = result.completed_depth;
        if result.completed_depth > 0 {
            completed = Some(result);
            break;
        }
    }
    let completed = completed.expect("Incremental calls must make progress");
    let paused = search.run(Budget {
        max_nodes: 0,
        max_millis: 0,
    });
    assert_eq!(paused.completed_depth, completed.completed_depth);
    assert_eq!(paused.best_move, completed.best_move);
    assert_eq!(paused.score, completed.score);
    assert_eq!(paused.nodes, completed.nodes);
}

#[test]
fn cyclic_games_finish_at_a_depth_limit_and_separate_rule_namespaces() {
    let cyclic = snapshot::decode(include_bytes!(
        "fixtures/multicolor-exact/full-board-draw.rcg"
    ))
    .unwrap();
    let result = analyse(&cyclic, &EvaluationMap::new(), 4, false);
    assert!(result.finished);
    assert_eq!(result.completed_depth, 4);
    assert_eq!(
        result.exact, None,
        "A finite heuristic search must not prove a cyclic draw"
    );
    assert_legal_analysis(&cyclic, &result);
    let a = opening();
    let mut b = a;
    b.remaining_cubies[0] -= 1;
    let mut known = EvaluationMap::new();
    known.insert(
        a.position_key(),
        Evaluation::Win {
            winner: 0,
            plies: 11,
        },
    );
    let untouched = known.clone();
    let result = analyse(&b, &known, 1, false);
    assert_ne!(a.position_key(), b.position_key());
    assert_eq!(result.exact, None);
    assert_eq!(known, untouched);
    let mut invalid = a;
    invalid.color_owners[0] = Some(2);
    assert!(
        Search::new(
            &invalid,
            &EvaluationMap::new(),
            Limits {
                max_depth: 2,
                max_records: 10
            }
        )
        .is_err()
    );
}
