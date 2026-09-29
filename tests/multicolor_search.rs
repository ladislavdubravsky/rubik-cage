//! Reachable positions with the requested four cubies of each color.
use rubik_cage::{
    core::snapshot,
    search::{
        Evaluation, EvaluationMap,
        general::{Budget, Search},
        retrograde,
    },
};

#[test]
fn full_four_each_endgames_close_even_with_no_proof_record_budget() {
    for (bytes, expected) in [
        (
            include_bytes!("fixtures/multicolor-exact/full-board-draw.rcg").as_slice(),
            Evaluation::Draw,
        ),
        (
            include_bytes!("fixtures/multicolor-exact/full-board-loss-in-6.rcg").as_slice(),
            Evaluation::Win {
                winner: 0,
                plies: 6,
            },
        ),
    ] {
        let state = snapshot::decode(bytes).unwrap();
        assert_eq!(state.inventories(), [4; 6]);
        assert_eq!(state.remaining_cubies, [0; 6]);
        assert!(state.outcome().is_none());
        let reference = retrograde::solve(
            &state,
            &EvaluationMap::new(),
            retrograde::Limits {
                max_states: 2048,
                max_edges: 14336,
            },
        )
        .unwrap();
        retrograde::verify(&reference.values).unwrap();
        assert_eq!(reference.values[&state.position_key()], expected);
        let mut search = Search::new(&state).unwrap();
        let mut complete = false;
        for _ in 0..100 {
            let batch = search
                .run(
                    &state,
                    Budget {
                        max_steps: 4000,
                        max_records: 0,
                        max_bytes: 16 * 1024 * 1024,
                        max_horizon: 6,
                    },
                )
                .unwrap();
            for (key, value) in &batch.values {
                assert_eq!(reference.values.get(key), Some(value));
            }
            if batch.complete {
                assert_eq!(batch.values[&state.position_key()], expected);
                for m in state.legal_moves() {
                    let mut child = state;
                    child.apply_move(m).unwrap();
                    assert_eq!(
                        batch.values.get(&child.position_key()),
                        reference.values.get(&child.position_key())
                    );
                }
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert!(
            search.proof.is_empty(),
            "Full graph provides values without finite-horizon proofs"
        );
    }
}

#[test]
fn reachable_four_each_tactics_have_verified_nontrivial_distances() {
    for (bytes, plies) in [
        (
            include_bytes!("fixtures/multicolor-exact/tactic-win-in-3.rcg").as_slice(),
            3,
        ),
        (
            include_bytes!("fixtures/multicolor-exact/tactic-win-in-5.rcg").as_slice(),
            5,
        ),
        (
            include_bytes!("fixtures/multicolor-exact/tactic-loss-in-4.rcg").as_slice(),
            4,
        ),
    ] {
        let state = snapshot::decode(bytes).unwrap();
        assert_eq!(state.inventories(), [4; 6]);
        assert!(state.outcome().is_none());
        let mut search = Search::new(&state).unwrap();
        let mut result = None;
        for _ in 0..100 {
            let batch = search
                .run(
                    &state,
                    Budget {
                        max_steps: 20_000,
                        max_records: 100_000,
                        max_bytes: 32 * 1024 * 1024,
                        max_horizon: 8,
                    },
                )
                .unwrap();
            result = batch.values.get(&state.position_key()).copied();
            if result.is_some() {
                break;
            }
        }
        assert_eq!(result, Some(Evaluation::Win { winner: 0, plies }));
        search.proof.verify().unwrap();
    }
}
