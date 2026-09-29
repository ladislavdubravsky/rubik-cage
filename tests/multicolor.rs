use rubik_cage::{
    app::utils::selected_color,
    core::{
        cubie::Cubie,
        game::{GameState, Outcome},
        r#move::{Layer, Move, Rotation},
        snapshot,
    },
    search::{
        Evaluation,
        bounded::{Budget, Search},
        cache::{Coverage, Table},
        packed::Space,
    },
};
use std::collections::HashSet;

fn drop(color: Cubie, x: usize, y: usize) -> Move {
    Move::Drop {
        color,
        column: (x, y),
    }
}
fn line(state: &mut GameState, color: Cubie, y: usize) {
    for x in 0..3 {
        state.cage.drop(color, (x, y)).unwrap();
        state.remaining_cubies[color as usize] -= 1;
    }
}

#[test]
fn preset_offers_every_owned_color_and_only_spends_the_chosen_stock() {
    let root = GameState::multicolor();
    root.validate().unwrap();
    assert_eq!(root.legal_moves().len(), 31);
    assert_eq!(root.inventories(), [4; 6]);
    assert_eq!(root.colors_for(0).count(), 3);
    assert_eq!(root.colors_for(1).count(), 3);
    let mut state = root;
    assert!(state.apply_move(drop(Cubie::Red, 0, 0)).is_err());
    assert_eq!(state, root);
    state.apply_move(drop(Cubie::Green, 0, 0)).unwrap();
    assert_eq!(state.remaining(Cubie::Green), 3);
    assert_eq!(state.remaining(Cubie::White), 4);
    assert_eq!(state.remaining(Cubie::Blue), 4);
    assert_eq!(state.player_to_move.id, 1);
    state.validate().unwrap();
    state.apply_move(drop(Cubie::Orange, 2, 2)).unwrap();
    assert_eq!(state.remaining(Cubie::Orange), 3);
    state.validate().unwrap();
}

#[test]
fn mixed_colors_do_not_win_but_each_monochromatic_color_does() {
    let mut mixed = GameState::multicolor();
    for (x, c) in [Cubie::White, Cubie::Blue, Cubie::Green]
        .into_iter()
        .enumerate()
    {
        mixed.cage.drop(c, (x, 0)).unwrap();
        mixed.remaining_cubies[c as usize] -= 1;
    }
    mixed.validate().unwrap();
    assert_eq!(mixed.outcome(), None);
    for color in Cubie::ALL {
        let mut state = GameState::multicolor();
        line(&mut state, color, 0);
        state.validate().unwrap();
        assert_eq!(
            state.outcome(),
            Some(Outcome::Win(state.owner(color).unwrap()))
        );
        assert_eq!(state.won().unwrap().1, color);
        assert!(state.legal_moves().is_empty());
        let before = state;
        assert!(state.apply_move(Move::Flip).is_err());
        assert_eq!(state, before);
    }
}

#[test]
fn two_winning_colors_of_one_owner_win_and_opposing_lines_draw() {
    let mut state = GameState::multicolor();
    line(&mut state, Cubie::White, 0);
    line(&mut state, Cubie::Blue, 2);
    state.validate().unwrap();
    assert_eq!(state.outcome(), Some(Outcome::Win(0)));
    let mut both = GameState::multicolor();
    line(&mut both, Cubie::White, 0);
    line(&mut both, Cubie::Red, 2);
    both.validate().unwrap();
    assert_eq!(both.outcome(), Some(Outcome::Draw));
    assert!(both.won().is_none());
}

#[test]
fn exhausted_stocks_keep_manipulations_legal_and_repair_selection() {
    let mut state = GameState::multicolor();
    state.remaining_cubies[Cubie::Green as usize] = 1;
    assert_eq!(
        selected_color(&state, 0, Some(Cubie::Green)),
        Some(Cubie::Green)
    );
    state.apply_move(drop(Cubie::Green, 0, 0)).unwrap();
    state.apply_move(Move::Flip).unwrap();
    assert_ne!(
        selected_color(&state, 0, Some(Cubie::Green)),
        Some(Cubie::Green)
    );
    assert!(!state.legal_moves().contains(&drop(Cubie::Green, 0, 1)));
    assert!(state.legal_moves().contains(&drop(Cubie::Blue, 0, 1)));
    state.remaining_cubies = [0; 6];
    assert_eq!(selected_color(&state, 0, Some(Cubie::Red)), None);
    assert!(
        state
            .legal_moves()
            .iter()
            .all(|m| !matches!(m, Move::Drop { .. }))
    );
    assert_eq!(state.legal_moves().len(), 6); // last flip still cannot be undone
}

#[test]
fn snapshots_restart_and_undo_preserve_each_color_and_restriction() {
    let root =
        GameState::with_colors(GameState::multicolor().color_owners, [3, 2, 1, 1, 2, 4]).unwrap();
    let mut state = root;
    let mut history = Vec::new();
    for m in [
        drop(Cubie::White, 0, 0),
        drop(Cubie::Yellow, 2, 2),
        drop(Cubie::Green, 0, 1),
        drop(Cubie::Orange, 2, 1),
        Move::RotateLayer {
            layer: Layer::Down,
            rotation: Rotation::Clockwise,
        },
        Move::Flip,
    ] {
        history.push(state);
        state.apply_move(m).unwrap();
        state.validate().unwrap();
    }
    let bytes = snapshot::encode(&state).unwrap();
    assert!(bytes.starts_with(b"RCGPOS02"));
    let imported = snapshot::decode(&bytes).unwrap();
    assert_eq!(imported, state);
    assert_eq!(imported.restarted(), root);
    assert!(!imported.legal_moves().contains(&Move::Flip));
    while let Some(previous) = history.pop() {
        state = previous;
    }
    assert_eq!(state, root);
    for end in 0..bytes.len() {
        assert!(snapshot::decode(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(snapshot::decode(&trailing).is_err());
    let mut future = bytes;
    future[8] = 99;
    assert!(snapshot::decode(&future).is_err());
}

#[test]
fn identity_tracks_colors_ownership_reserves_and_player_relabeling() {
    let root = GameState::multicolor();
    let mut a = root;
    a.apply_move(drop(Cubie::White, 0, 0)).unwrap();
    let mut b = root;
    b.apply_move(drop(Cubie::Green, 0, 0)).unwrap();
    assert_ne!(a.position_key(), b.position_key());
    let mut b = a;
    b.color_owners
        .swap(Cubie::White as usize, Cubie::Yellow as usize);
    assert_ne!(a.position_key(), b.position_key());
    let mut b = a;
    b.remaining_cubies
        .swap(Cubie::White as usize, Cubie::Blue as usize);
    assert_ne!(a.position_key(), b.position_key());
    assert_ne!(root.position_key(), GameState::new(9, 9).position_key());
    let mut terminal = root;
    line(&mut terminal, Cubie::Green, 0);
    for state in [root, a, terminal] {
        let key = state.position_key();
        let swapped = key.swapped_players();
        swapped.validate().unwrap();
        assert_eq!(swapped.swapped_players(), key);
        let successors = |state: GameState| -> HashSet<_> {
            state
                .legal_moves()
                .into_iter()
                .map(|m| {
                    let mut child = state;
                    child.apply_move(m).unwrap();
                    child.position_key()
                })
                .collect()
        };
        assert_eq!(
            successors(state)
                .into_iter()
                .map(|k| k.swapped_players())
                .collect::<HashSet<_>>(),
            successors(swapped.to_state())
        );
        if let Some(Outcome::Win(id)) = state.outcome() {
            assert_eq!(swapped.to_state().outcome(), Some(Outcome::Win(1 - id)));
        }
    }
}

#[test]
fn invalid_configuration_and_legacy_backend_misuse_are_rejected() {
    let root = GameState::multicolor();
    let mut owners = root.color_owners;
    owners[0] = Some(2);
    assert!(GameState::with_colors(owners, [3; 6]).is_err());
    owners[0] = None;
    assert!(GameState::with_colors(owners, [3; 6]).is_err());
    assert!(GameState::with_colors(root.color_owners, [24; 6]).is_err());
    assert!(GameState::single_color([Cubie::Blue; 2], [3, 3]).is_err());
    let mut invalid = root;
    invalid.last_move = Some(drop(Cubie::Green, 0, 0));
    assert!(invalid.validate().is_err()); // P1 to move means the previous drop must be P2's
    assert!(Space::new(&root).is_err());
    assert!(Search::new(&root, Budget::default()).is_err());
    let single = GameState::new(9, 9);
    assert!(Space::new(&single).unwrap().try_encode(&root).is_err());
    let table = Table {
        roots: vec![root.position_key()],
        coverage: Coverage::Subset,
        values: Default::default(),
    };
    assert!(table.encode().is_err());
    let mut winning = root;
    for x in 0..2 {
        winning.cage.drop(Cubie::Green, (x, 0)).unwrap();
        winning.remaining_cubies[Cubie::Green as usize] -= 1;
    }
    winning.apply_move(drop(Cubie::Green, 2, 0)).unwrap();
    assert_eq!(
        Evaluation::terminal(&winning),
        Some(Evaluation::Win {
            winner: 0,
            plies: 0
        })
    );
}
