//! Exact search identity. Zobrist hashes remain only for legacy position decoding.
use super::{
    cage::Cage,
    game::{GameState, Player},
    r#move::{Move, Rotation},
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode, Serialize, Deserialize,
)]
pub struct PositionKey {
    cage: Cage,
    owners: [Option<u8>; 6],
    remaining: [u8; 6],
    turn: u8,
    forbidden: Option<Move>,
}

impl PositionKey {
    pub fn new(state: &GameState) -> Self {
        let raw = Self::unoriented(state);
        (0..8)
            .map(|symmetry| raw.transformed(symmetry))
            .max()
            .unwrap()
    }

    pub(crate) fn unoriented(state: &GameState) -> Self {
        Self {
            cage: state.cage,
            owners: state.color_owners,
            remaining: state.remaining_cubies,
            turn: state.player_to_move.id,
            forbidden: state.last_move.and_then(Move::inverse),
        }
    }

    /// Relabel players and their complete rule state. Single-color games retain
    /// their historical board/stock color swap; multi-color games swap ownership.
    /// The turn and winning player swap in either case; restrictions stay intact.
    pub fn swapped_players(mut self) -> Self {
        if let Some([a, b]) = self.to_state().single_colors() {
            // Preserve the original two-color equivalence and bundled cache reuse.
            for color in self.cage.grid.iter_mut().flatten().flatten().flatten() {
                *color = if *color == a { b } else { a };
            }
            self.remaining.swap(a as usize, b as usize);
        } else {
            // General player relabeling keeps exact color labels and stocks intact.
            for owner in self.owners.iter_mut().flatten() {
                *owner ^= 1;
            }
        }
        self.turn ^= 1;
        (0..8)
            .map(|symmetry| self.transformed(symmetry))
            .max()
            .unwrap()
    }

    pub fn turn(&self) -> u8 {
        self.turn
    }

    /// The rule state in canonical orientation. Irrelevant drop history is discarded.
    pub fn to_state(&self) -> GameState {
        let players = [Player { id: 0 }, Player { id: 1 }];
        let mut state = GameState {
            cage: self.cage,
            players,
            color_owners: self.owners,
            remaining_cubies: self.remaining,
            player_to_move: players[self.turn as usize],
            zobrist_hash: 0,
            last_move: self.forbidden.and_then(Move::inverse),
        };
        state.rebuild_zobrist_hash();
        state
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.turn > 1 || matches!(self.forbidden, Some(Move::Drop { .. })) {
            return Err("Invalid position key");
        }
        let state = self.to_state();
        state.validate()?;
        if state.position_key() != *self {
            return Err("Noncanonical position key");
        }
        Ok(())
    }

    /// Four rotations and their reflections; transform the restriction before tie-breaking.
    pub(crate) fn transformed(mut self, symmetry: usize) -> Self {
        let reflected = symmetry >= 4;
        let mut cage = Cage::new();
        for x in 0..3 {
            for y in 0..3 {
                let (mut nx, mut ny) = (if reflected { 2 - x } else { x }, y);
                for _ in 0..symmetry % 4 {
                    (nx, ny) = (ny, 2 - nx);
                }
                cage.grid[nx][ny] = self.cage.grid[x][y];
            }
        }
        self.cage = cage;
        if reflected && let Some(Move::RotateLayer { layer, rotation }) = self.forbidden {
            self.forbidden = Some(Move::RotateLayer {
                layer,
                rotation: match rotation {
                    Rotation::Clockwise => Rotation::CounterClockwise,
                    Rotation::CounterClockwise => Rotation::Clockwise,
                    Rotation::HalfTurn => Rotation::HalfTurn,
                },
            });
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::super::{game::Outcome, r#move::Layer};
    use super::*;
    use crate::core::cubie::Cubie;
    use std::collections::{HashSet, VecDeque};

    fn successors(state: GameState) -> HashSet<PositionKey> {
        state
            .legal_moves()
            .into_iter()
            .map(|m| {
                let mut next = state;
                next.apply_move(m).unwrap();
                next.position_key()
            })
            .collect()
    }

    #[test]
    fn swapping_players_preserves_all_legal_futures() {
        for (m, n) in [(1, 1), (3, 1), (3, 2)] {
            let root = GameState::new(m, n);
            let graph =
                crate::search::retrograde::solve(&root, &Default::default(), Default::default())
                    .unwrap();
            for &key in graph.values.keys() {
                let swapped = key.swapped_players();
                swapped.validate().unwrap();
                assert_eq!(swapped.swapped_players(), key);
                assert_eq!(
                    successors(key.to_state())
                        .into_iter()
                        .map(PositionKey::swapped_players)
                        .collect::<HashSet<_>>(),
                    successors(swapped.to_state())
                );
                let expected = key.to_state().outcome().map(|o| match o {
                    Outcome::Win(w) => Outcome::Win(1 - w),
                    Outcome::Draw => Outcome::Draw,
                });
                assert_eq!(swapped.to_state().outcome(), expected);
            }
        }
    }

    #[test]
    fn identity_covers_inventory_turn_colors_and_restrictions() {
        let initial = GameState::new(3, 1);
        assert_ne!(initial.position_key(), GameState::new(4, 1).position_key());
        let mut flip = initial;
        flip.apply_move(Move::Flip).unwrap();
        let mut rotation = initial;
        rotation
            .apply_move(Move::RotateLayer {
                layer: Layer::Up,
                rotation: Rotation::Clockwise,
            })
            .unwrap();
        assert_eq!(flip.zobrist_hash, rotation.zobrist_hash); // old identity aliases
        assert_ne!(flip.position_key(), rotation.position_key());
        assert_ne!(flip.position_key(), initial.position_key());
        let colors = GameState::single_color([Cubie::Yellow, Cubie::Red], [3, 1]).unwrap();
        assert_ne!(colors.position_key(), initial.position_key());
    }

    #[test]
    fn restriction_breaks_symmetric_board_ties() {
        let mut state = GameState::new(1, 1);
        state
            .apply_move(Move::RotateLayer {
                layer: Layer::Up,
                rotation: Rotation::Clockwise,
            })
            .unwrap();
        let reflected = PositionKey::unoriented(&state).transformed(4).to_state();
        assert_eq!(state.position_key(), reflected.position_key());
        assert_eq!(successors(state), successors(reflected));
    }

    #[test]
    fn canonicalization_preserves_transitions_and_outcomes() {
        let initial = GameState::new(3, 1);
        let mut todo = VecDeque::from([initial]);
        let mut seen = HashSet::from([initial.position_key()]);
        for _ in 0..128 {
            let Some(state) = todo.pop_front() else {
                break;
            };
            let key = state.position_key();
            assert_eq!(key, key.to_state().position_key());
            let expected = successors(state);
            for symmetry in 0..8 {
                let transformed = PositionKey::unoriented(&state)
                    .transformed(symmetry)
                    .to_state();
                assert_eq!(key, transformed.position_key());
                assert_eq!(state.outcome(), transformed.outcome());
                assert_eq!(expected, successors(transformed));
            }
            for m in state.legal_moves() {
                let mut plain = state;
                plain.apply_move(m).unwrap();
                let mut normalized = state;
                normalized.apply_move_normalize(m).unwrap();
                assert_eq!(plain.position_key(), normalized.position_key());
                if seen.insert(plain.position_key()) {
                    todo.push_back(plain);
                }
            }
        }
        let mut both = GameState::new(0, 0);
        both.cage = ".........,.........,RBBR.BR.B".parse().unwrap();
        for symmetry in 0..8 {
            let transformed = PositionKey::unoriented(&both)
                .transformed(symmetry)
                .to_state();
            assert_eq!(transformed.outcome(), Some(Outcome::Draw));
            assert!(transformed.legal_moves().is_empty());
            assert!(transformed.won().is_none());
        }
    }

    #[test]
    fn legal_rotation_creating_both_lines_ends_in_draw() {
        let mut state = GameState::new(0, 1);
        state.cage = ".........,R.......B,BB.B...RR".parse().unwrap();
        state.validate().unwrap();
        assert_eq!(state.outcome(), None);
        state
            .apply_move(Move::RotateLayer {
                layer: Layer::Down,
                rotation: Rotation::Clockwise,
            })
            .unwrap();
        assert_eq!(state.outcome(), Some(Outcome::Draw));
        assert!(state.legal_moves().is_empty());
        let terminal = state;
        assert!(state.apply_move(Move::Flip).is_err());
        assert_eq!(state, terminal);
    }

    #[test]
    fn multicolor_spatial_symmetries_preserve_transitions_and_outcomes() {
        let mut state = GameState::multicolor();
        for ply in 0..24 {
            let expected = successors(state);
            for symmetry in 0..8 {
                let transformed = PositionKey::unoriented(&state)
                    .transformed(symmetry)
                    .to_state();
                assert_eq!(state.position_key(), transformed.position_key());
                assert_eq!(state.outcome(), transformed.outcome());
                assert_eq!(expected, successors(transformed));
            }
            let moves = state.legal_moves();
            if moves.is_empty() {
                break;
            }
            state.apply_move(moves[(ply * 7) % moves.len()]).unwrap();
        }
    }

    #[test]
    fn illegal_moves_leave_state_unchanged() {
        let mut state = GameState::new(1, 1);
        for m in [
            Move::Drop {
                color: Cubie::Red,
                column: (0, 0),
            },
            Move::Drop {
                color: Cubie::Blue,
                column: (3, 0),
            },
            Move::Drop {
                color: Cubie::Blue,
                column: (1, 1),
            },
        ] {
            let before = state;
            assert!(state.apply_move(m).is_err());
            assert_eq!(state, before);
        }
        state.apply_move_normalize(Move::Flip).unwrap();
        let before = state;
        assert!(state.apply_move_normalize(Move::Flip).is_err());
        assert_eq!(state, before);
    }
}
