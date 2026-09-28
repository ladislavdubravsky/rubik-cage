use super::*;
use crate::core::position::PositionKey;

// Deliberately separate synchronous recurrence: no predecessor counts or queue.
fn reference(nodes: &[Node]) -> Vec<Evaluation> {
    let mut values = vec![None; nodes.len()];
    for _ in 0..10_000 {
        let next: Vec<_> = nodes
            .iter()
            .map(|n| {
                if n.seed.is_some() {
                    return n.seed;
                }
                let children: Vec<_> = n.children.iter().map(|&id| values[id]).collect();
                if let Some(d) = children
                    .iter()
                    .flatten()
                    .filter(|v: &&Evaluation| v.winner() == Some(n.owner))
                    .filter_map(|v| v.plies())
                    .min()
                {
                    Some(Evaluation::Win {
                        winner: n.owner,
                        plies: d + 1,
                    })
                } else if !children.is_empty()
                    && children
                        .iter()
                        .all(|v| v.is_some_and(|v| v.winner() == Some(1 - n.owner)))
                {
                    Some(Evaluation::Win {
                        winner: 1 - n.owner,
                        plies: children
                            .iter()
                            .flatten()
                            .filter_map(|v| v.plies())
                            .max()
                            .unwrap()
                            + 1,
                    })
                } else {
                    None
                }
            })
            .collect();
        if next == values {
            return next
                .into_iter()
                .map(|v| v.unwrap_or(Evaluation::Draw))
                .collect();
        }
        values = next;
    }
    panic!("Reference did not converge");
}

#[test]
fn exhaustive_small_cyclic_graphs_match_reference() {
    for second_seed in [
        Evaluation::Win {
            winner: 1,
            plies: 0,
        },
        Evaluation::Draw,
    ] {
        for owners in 0..4 {
            for a in 1..16 {
                for b in 1..16 {
                    let mut nodes = vec![
                        Node {
                            owner: owners & 1,
                            children: (0..4).filter(|i| a & (1 << i) != 0).collect(),
                            seed: None,
                        },
                        Node {
                            owner: (owners >> 1) & 1,
                            children: (0..4).filter(|i| b & (1 << i) != 0).collect(),
                            seed: None,
                        },
                        Node {
                            owner: 1,
                            children: vec![],
                            seed: Some(Evaluation::Win {
                                winner: 0,
                                plies: 0,
                            }),
                        },
                        Node {
                            owner: 0,
                            children: vec![],
                            seed: Some(second_seed),
                        },
                    ];
                    let expected = reference(&nodes);
                    assert_eq!(propagate(&nodes).unwrap(), expected);
                    for node in &mut nodes {
                        node.children.reverse();
                    }
                    assert_eq!(propagate(&nodes).unwrap(), expected);
                }
            }
        }
    }
}

#[test]
fn weighted_boundaries_keep_distances_and_loser_delays() {
    let nodes = vec![
        Node {
            owner: 0,
            children: vec![1, 2],
            seed: None,
        },
        Node {
            owner: 1,
            children: vec![],
            seed: Some(Evaluation::Win {
                winner: 0,
                plies: 12,
            }),
        },
        Node {
            owner: 1,
            children: vec![],
            seed: Some(Evaluation::Win {
                winner: 0,
                plies: 2,
            }),
        },
        Node {
            owner: 1,
            children: vec![1, 2, 2],
            seed: None,
        },
    ];
    let result = propagate(&nodes).unwrap();
    assert_eq!(result, reference(&nodes));
    assert_eq!(
        result[0],
        Evaluation::Win {
            winner: 0,
            plies: 3
        }
    );
    assert_eq!(
        result[3],
        Evaluation::Win {
            winner: 0,
            plies: 13
        }
    );
}

fn reference_game(initial: GameState, symmetry: bool) -> EvaluationMap {
    let key = |s: &GameState| {
        if symmetry {
            s.position_key()
        } else {
            PositionKey::unoriented(s)
        }
    };
    let mut keys = vec![key(&initial)];
    let mut ids = HashMap::from([(keys[0], 0)]);
    let mut nodes = Vec::new();
    let mut i = 0;
    while i < keys.len() {
        let state = keys[i].to_state();
        let mut children = Vec::new();
        for m in state.legal_moves() {
            let mut child = state;
            child.apply_move(m).unwrap();
            let child_key = key(&child);
            let next_id = keys.len();
            let id = *ids.entry(child_key).or_insert(next_id);
            if id == next_id {
                keys.push(child_key);
            }
            children.push(id);
        }
        nodes.push(Node {
            owner: state.player_to_move.id,
            children,
            seed: Evaluation::terminal(&state),
        });
        i += 1;
    }
    keys.into_iter().zip(reference(&nodes)).collect()
}

#[test]
fn small_games_reference_certificates_reroots_and_boundaries() {
    for (p1, p2) in [(1, 1), (3, 0), (0, 3), (3, 1)] {
        let initial = GameState::new(p1, p2);
        let result = solve(&initial, &EvaluationMap::new(), Limits::default()).unwrap();
        assert_eq!(result.values, reference_game(initial, true));
        if (p1, p2) == (1, 1) {
            assert_eq!(result.values[&initial.position_key()], Evaluation::Draw);
        }
        verify(&result.values).unwrap();
        let mut entries: Vec<_> = result.values.iter().map(|(k, v)| (*k, *v)).collect();
        entries.sort_by_key(|(key, _)| *key);
        for (key, value) in entries.iter().step_by((entries.len() / 5).max(1)) {
            let fresh = solve(&key.to_state(), &EvaluationMap::new(), Limits::default()).unwrap();
            assert_eq!(fresh.values[key], *value);
            for (k, v) in fresh.values {
                assert_eq!(result.values[&k], v);
            }
        }
        let boundaries = entries.into_iter().skip(1).step_by(3).collect();
        let cached = solve(&initial, &boundaries, Limits::default()).unwrap();
        for (k, v) in cached.values {
            assert_eq!(result.values[&k], v);
        }
    }
}

#[test]
fn symmetry_reduction_matches_unreduced_game() {
    for initial in [
        GameState::new(3, 0),
        GameState::new(1, 1),
        GameState::new(3, 1),
    ] {
        let canonical = solve(&initial, &EvaluationMap::new(), Limits::default()).unwrap();
        for (key, value) in reference_game(initial, false) {
            assert_eq!(canonical.values[&key.to_state().position_key()], value);
        }
    }
}

#[test]
fn incomplete_is_not_draw_and_limits_are_enforced() {
    for limits in [
        Limits {
            max_states: 1,
            max_edges: 100,
        },
        Limits {
            max_states: 100,
            max_edges: 0,
        },
    ] {
        assert!(matches!(
            solve(&GameState::new(3, 1), &EvaluationMap::new(), limits),
            Err(SolveError::LimitReached(_))
        ));
    }
}

#[test]
fn verifier_rejects_corrupt_values_and_missing_children() {
    let state = GameState::new(3, 0);
    let mut values = solve(&state, &EvaluationMap::new(), Limits::default())
        .unwrap()
        .values;
    values.insert(state.position_key(), Evaluation::Draw);
    assert!(verify(&values).is_err());
    let mut values = EvaluationMap::new();
    values.insert(state.position_key(), Evaluation::Draw);
    assert!(verify(&values).is_err());
}
