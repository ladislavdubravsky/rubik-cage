use crate::core::{
    cage::Cage,
    cubie::Cubie,
    line::Line,
    r#move::{Layer, Move, Rotation},
    zobrist,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub color: Cubie,
    pub id: u8,
}

// TODO: enable more than 2 players and more than 1 color per player
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub cage: Cage,
    pub players: [Player; 2],
    pub remaining_cubies: [u8; 2],
    pub player_to_move: Player,
    pub zobrist_hash: u64,
    pub last_move: Option<Move>,
}

impl GameState {
    pub fn new(p1_cubies: u8, p2_cubies: u8) -> Self {
        let players = [
            Player {
                color: Cubie::Blue,
                id: 0,
            },
            Player {
                color: Cubie::Red,
                id: 1,
            },
        ];

        Self {
            cage: Cage::new(),
            players,
            remaining_cubies: [p1_cubies, p2_cubies],
            player_to_move: players[0],
            zobrist_hash: 0,
            last_move: None,
        }
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();

        // Drops into non-full columns by the player to move. Allowed if the player still has
        // cubies to drop.
        if self.remaining_cubies[self.player_to_move.id as usize] > 0 {
            for x in 0..3 {
                for y in 0..3 {
                    if Cage::is_center(x, y) {
                        continue;
                    }
                    if self.cage.grid[x][y][2].is_none() {
                        moves.push(Move::Drop {
                            color: self.player_to_move.color,
                            column: (x, y),
                        });
                    }
                }
            }
        }

        // Flip: allowed if not inverting the previous move
        if self.last_move != Some(Move::Flip) {
            moves.push(Move::Flip);
        }

        // Rotations: allowed if not inverting the previous move
        for layer in [Layer::Down, Layer::Equator, Layer::Up] {
            for rotation in [Rotation::Clockwise, Rotation::CounterClockwise] {
                let r#move = Move::RotateLayer { layer, rotation };
                if self.last_move != r#move.inverse() {
                    moves.push(r#move);
                }
            }
        }

        moves
    }

    fn advance_player_to_move(&mut self) {
        self.player_to_move = if self.player_to_move.id == 0 {
            self.players[1]
        } else {
            self.players[0]
        };
    }

    pub fn apply_move(&mut self, r#move: Move) -> Result<(), &'static str> {
        let current_player = self.player_to_move;
        self.advance_player_to_move();
        self.last_move = Some(r#move);

        match r#move {
            Move::Drop { color, column } => {
                let z = self.cage.drop(color, column)?;
                self.zobrist_hash ^= zobrist::POS_COLOR[color as usize][column.0][column.1][z];
                self.zobrist_hash ^= *zobrist::P2_TO_MOVE;
                self.remaining_cubies[current_player.id as usize] -= 1;
            }
            Move::Flip => {
                self.cage.flip();
                self.rebuild_zobrist_hash();
            }
            Move::RotateLayer { layer, rotation } => {
                self.cage.rotate_layer(layer, rotation);
                self.rebuild_zobrist_hash();
            }
        }

        Ok(())
    }

    pub fn won(&self) -> Option<(Player, Line)> {
        if let Some((cubie, line)) = self.cage.has_line() {
            for player in &self.players {
                if player.color == cubie {
                    return Some((*player, line));
                }
            }
        }
        None
    }

    pub fn normalize(&mut self) {
        let reflection_happened = self.cage.normalize();
        if reflection_happened {
            self.last_move = match self.last_move {
                Some(Move::Flip) => Some(Move::Flip),
                Some(Move::Drop { color, column }) => Some(Move::Drop {
                    color,
                    column: (2 - column.0, column.1),
                }),
                Some(Move::RotateLayer { layer, rotation }) => Some(Move::RotateLayer {
                    layer,
                    rotation: match rotation {
                        Rotation::Clockwise => Rotation::CounterClockwise,
                        Rotation::CounterClockwise => Rotation::Clockwise,
                        Rotation::HalfTurn => Rotation::HalfTurn,
                    },
                }),
                None => None,
            }
        }
        self.rebuild_zobrist_hash();
    }

    pub fn apply_move_normalize(&mut self, r#move: Move) -> Result<(), &'static str> {
        let current_player = self.player_to_move;
        self.advance_player_to_move();

        match r#move {
            Move::Drop { color, column } => {
                self.cage.drop(color, column)?;
                self.remaining_cubies[current_player.id as usize] -= 1;
            }
            Move::Flip => self.cage.flip(),
            Move::RotateLayer { layer, rotation } => self.cage.rotate_layer(layer, rotation),
        }

        self.normalize();

        Ok(())
    }

    fn rebuild_zobrist_hash(&mut self) {
        self.zobrist_hash = 0;
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    if let Some(cubie) = self.cage.grid[x][y][z] {
                        self.zobrist_hash ^= zobrist::POS_COLOR[cubie as usize][x][y][z];
                    }
                }
            }
        }
        if self.player_to_move.id == 1 {
            self.zobrist_hash ^= *zobrist::P2_TO_MOVE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legal_moves_initial_state() {
        let game = GameState::new(4, 4);
        let legal_moves = game.legal_moves();
        assert!(legal_moves.len() == 15);
    }

    #[test]
    fn test_full_column_drop_illegal() {
        let mut game = GameState::new(4, 4);
        for _ in 0..3 {
            game.cage.drop(game.player_to_move.color, (0, 0)).unwrap();
        }

        let legal_moves = game.legal_moves();
        assert!(legal_moves.len() == 14);
        assert!(!legal_moves.contains(&Move::Drop {
            color: game.player_to_move.color,
            column: (0, 0)
        }));
    }

    #[test]
    fn test_out_of_turn_drop_illegal() {
        let mut game = GameState::new(4, 4);
        game.apply_move(Move::Drop {
            color: game.players[0].color,
            column: (1, 2),
        })
        .unwrap();

        // Now it's player 1's turn, so player 0 cannot drop a cubie
        assert!(!game.legal_moves().contains(&Move::Drop {
            color: game.players[0].color,
            column: (1, 2)
        }));
        // Player 1 can
        assert!(game.legal_moves().contains(&Move::Drop {
            color: game.players[1].color,
            column: (1, 2)
        }));
    }

    #[test]
    fn test_inverting_moves_illegal() {
        let mut game = GameState::new(4, 4);
        game.apply_move(Move::Flip).unwrap();
        assert!(!game.legal_moves().contains(&Move::Flip));

        game.apply_move(Move::RotateLayer {
            layer: Layer::Down,
            rotation: Rotation::Clockwise,
        })
        .unwrap();
        assert!(!game.legal_moves().contains(&Move::RotateLayer {
            layer: Layer::Down,
            rotation: Rotation::CounterClockwise,
        }));
    }

    #[test]
    fn test_no_drops_after_cubies_spent() {
        let mut game = GameState::new(1, 1);
        game.apply_move(Move::Drop {
            color: game.player_to_move.color,
            column: (0, 0),
        })
        .unwrap();
        game.apply_move(Move::Flip).unwrap();

        // First player's turn again, but has no more cubies
        assert!(
            !game
                .legal_moves()
                .iter()
                .any(|m| matches!(m, Move::Drop { .. }))
        );
    }

    #[test]
    fn test_zobrist_single_drop() {
        let mut game = GameState::new(2, 2);
        game.apply_move(Move::Drop {
            color: game.player_to_move.color,
            column: (0, 0),
        })
        .unwrap();
        let zobrist1 = game.zobrist_hash;

        game.rebuild_zobrist_hash();
        let zobrist2 = game.zobrist_hash;

        assert_eq!(zobrist1, zobrist2);
    }

    #[test]
    fn test_zobrist_drop_and_rotate() {
        let mut game1 = GameState::new(2, 2);
        game1
            .apply_move(Move::Drop {
                color: game1.player_to_move.color,
                column: (0, 0),
            })
            .unwrap();

        let mut game2 = GameState::new(2, 2);
        game2
            .apply_move(Move::Drop {
                color: game2.player_to_move.color,
                column: (2, 0),
            })
            .unwrap();
        game2
            .apply_move(Move::RotateLayer {
                layer: Layer::Down,
                rotation: Rotation::CounterClockwise,
            })
            .unwrap();

        // Same cage, different player to move
        assert_ne!(game1.zobrist_hash, game2.zobrist_hash);

        // Pass the turn to player 1
        game1
            .apply_move(Move::RotateLayer {
                layer: Layer::Up,
                rotation: Rotation::Clockwise,
            })
            .unwrap();
        assert_eq!(game1.zobrist_hash, game2.zobrist_hash);
    }
}
