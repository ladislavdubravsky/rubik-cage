//! Fixed pre-migration bytes and behavior gates for the specialized solver.
use rubik_cage::{
    core::{
        cubie::Cubie,
        game::GameState,
        r#move::{Layer, Move, Rotation},
        snapshot,
    },
    search::{
        Evaluation,
        bounded::{Budget, Proof, Search},
        cache::Table,
        packed::Space,
    },
};
use std::collections::HashSet;

const ROTATION: &[u8] = include_bytes!("fixtures/single-color-v1/rotation.position.bin");
const ROTATION_RAW: &[u8] = include_bytes!("fixtures/single-color-v1/rotation.raw.bin");
const FLIP: &[u8] = include_bytes!("fixtures/single-color-v1/flip.position.bin");
const FLIP_RAW: &[u8] = include_bytes!("fixtures/single-color-v1/flip.raw.bin");
const PROOF_V1: &[u8] = include_bytes!("fixtures/single-color-v1/horizon.proof-v1.bin");
const PROOF_V2: &[u8] = include_bytes!("fixtures/single-color-v1/horizon.proof-v2.bin");

fn saved_state() -> GameState {
    let mut state = GameState::single_color([Cubie::White, Cubie::Green], [20, 4]).unwrap();
    for m in [
        Move::Drop {
            color: Cubie::White,
            column: (0, 0),
        },
        Move::Drop {
            color: Cubie::Green,
            column: (2, 1),
        },
        Move::RotateLayer {
            layer: Layer::Down,
            rotation: Rotation::Clockwise,
        },
    ] {
        state.apply_move(m).unwrap();
    }
    state
}

#[test]
fn fixed_positions_preserve_stocks_colors_turn_restriction_and_rebuild_hash() {
    let mut expected = saved_state();
    for (versioned, raw) in [(ROTATION, ROTATION_RAW), (FLIP, FLIP_RAW)] {
        if versioned == FLIP {
            expected.apply_move(Move::Flip).unwrap();
        }
        for bytes in [versioned, raw] {
            let state = snapshot::decode(bytes).unwrap();
            assert_eq!(state, expected);
            assert_eq!(
                [state.remaining(Cubie::White), state.remaining(Cubie::Green)],
                [19, 3]
            );
            assert_eq!(Space::new(&state).unwrap().totals, [20, 4]);
            assert_ne!(state.zobrist_hash, 123);
            assert!(
                !state
                    .legal_moves()
                    .contains(&state.last_move.unwrap().inverse().unwrap())
            );
            assert_eq!(snapshot::encode(&state).unwrap(), versioned);
        }
    }
}

#[test]
fn fixed_readers_reject_truncation_trailing_data_and_unknown_versions() {
    for bytes in [ROTATION, ROTATION_RAW, FLIP, FLIP_RAW] {
        for end in 0..bytes.len() {
            assert!(snapshot::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(snapshot::decode(&trailing).is_err());
    }
    let mut unsupported = ROTATION.to_vec();
    unsupported[8] = 2;
    assert!(snapshot::decode(&unsupported).is_err());
    for bytes in [PROOF_V1, PROOF_V2] {
        for end in 0..bytes.len() {
            assert!(Proof::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(Proof::decode(&trailing).is_err());
        let mut unsupported = bytes.to_vec();
        unsupported[8] = 2;
        assert!(Proof::decode(&unsupported).is_err());
        unsupported[7] = b'9';
        assert!(Proof::decode(&unsupported).is_err());
    }
    assert!(Table::decode(include_bytes!("../assets/eval.bin")).is_err());
}

#[test]
fn both_fixed_proof_versions_preserve_their_claims() {
    let root = GameState::new(3, 0).position_key();
    let v1 = Proof::decode(PROOF_V1).unwrap();
    let v2 = Proof::decode(PROOF_V2).unwrap();
    assert_eq!(
        v1.exact_values()[&root],
        Evaluation::Win {
            winner: 0,
            plies: 5
        }
    );
    assert_eq!(v1.bounds, v2.bounds);
    assert_eq!(v1.exact_values(), v2.exact_values());
    assert_eq!(v2.encode().unwrap(), PROOF_V2);
    assert_eq!(
        Proof::decode(&v1.encode().unwrap()).unwrap().bounds,
        v1.bounds
    );
}

#[test]
fn shipped_tables_keep_their_bytes_and_all_full_inventory_proof_values() {
    let full_bytes = include_bytes!("../assets/eval-v1.bin");
    let full = Table::decode(full_bytes).unwrap();
    assert_eq!(full.values.len(), 122_727);
    assert!(
        full.encode().unwrap() == full_bytes,
        "Full table wire bytes changed"
    );
    let proof = Proof::decode(include_bytes!("../assets/eval-v1.proof.bin")).unwrap();
    let certified = proof.exact_values();
    for (key, value) in &full.values {
        assert_eq!(certified.get(key), Some(value));
    }
    let root = GameState::new(12, 12);
    assert_eq!(
        full.values[&root.position_key()],
        Evaluation::Win {
            winner: 0,
            plies: 11
        }
    );
    assert_eq!(root.legal_moves().len(), 15);
    for m in root.legal_moves() {
        let mut child = root;
        child.apply_move(m).unwrap();
        let expected = match m {
            Move::Drop { column: (x, y), .. } if x == 1 || y == 1 => Evaluation::Draw,
            Move::Drop { .. } => Evaluation::Win {
                winner: 0,
                plies: 10,
            },
            _ => Evaluation::Win {
                winner: 1,
                plies: 11,
            },
        };
        assert_eq!(full.values[&child.position_key()], expected);
    }
    let small_bytes = include_bytes!("../assets/eval-3-1.bin");
    let small = Table::decode(small_bytes).unwrap();
    // This older asset records "exact-retrograde-v1" as its generator. Both the
    // original and current writer use "verified-exact-v1"; compare all game data.
    let rewritten = small.encode().unwrap();
    let roundtrip = Table::decode(&rewritten).unwrap();
    assert_eq!(roundtrip.roots, small.roots);
    assert_eq!(roundtrip.coverage, small.coverage);
    assert_eq!(roundtrip.values, small.values);
    assert!(roundtrip.encode().unwrap() == rewritten);
    assert_eq!(
        small.values[&GameState::new(3, 1).position_key()],
        Evaluation::Win {
            winner: 0,
            plies: 9
        }
    );
}

#[test]
fn specialized_adapter_matches_core_for_every_color_pair_and_restriction() {
    let colors = [
        Cubie::White,
        Cubie::Yellow,
        Cubie::Red,
        Cubie::Orange,
        Cubie::Blue,
        Cubie::Green,
    ];
    for a in colors {
        for b in colors {
            if a == b {
                continue;
            }
            let root = GameState::single_color([a, b], [3, 1]).unwrap();
            let space = Space::new(&root).unwrap();
            let mut states = vec![root];
            for m in root.legal_moves() {
                let mut child = root;
                child.apply_move(m).unwrap();
                states.push(child);
            }
            for state in states {
                let packed = space.try_encode(&state).unwrap();
                assert_eq!(space.decode(packed).position_key(), state.position_key());
                let expected: HashSet<_> = state
                    .legal_moves()
                    .into_iter()
                    .map(|m| {
                        let mut next = state;
                        next.apply_move(m).unwrap();
                        space.try_encode(&next).unwrap()
                    })
                    .collect();
                let children = space.children(packed);
                assert_eq!(
                    expected,
                    children.items[..children.len].iter().copied().collect()
                );
            }
        }
    }
}

#[test]
fn specialized_adapter_rejects_invalid_and_mismatched_games_without_panicking() {
    let root = GameState::new(3, 1);
    let space = Space::new(&root).unwrap();
    let wrong_color = GameState::single_color([Cubie::Yellow, Cubie::Red], [3, 1]).unwrap();
    let mut invalid = root;
    invalid.player_to_move.id = 2;
    let mut foreign_piece = root;
    foreign_piece.cage.drop(Cubie::Green, (0, 0)).unwrap();
    for state in [GameState::new(4, 1), wrong_color, invalid, foreign_piece] {
        assert!(space.try_encode(&state).is_err());
        assert!(
            Search::new(&root, Budget::default())
                .unwrap()
                .exact(&state, 3)
                .is_err()
        );
    }
}
