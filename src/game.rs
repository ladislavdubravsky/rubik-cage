use crate::{
    cage::Cage,
    cubie::Cubie,
    r#move::{Layer, Move, Rotation},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Player {
    pub color: Cubie,
    pub id: u8,
}

// TODO: enable more than 2 players and more than 1 color per player
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GameState {
    pub cage: Cage,
    pub players: [Player; 2],
    pub remaining_cubies: [u8; 2],
    pub player_to_move: Player,
}

impl GameState {
    pub fn new(p1_cubies: u8, p2_cubies: u8) -> Self {
        let players = [
            Player {
                color: Cubie::Blue,
                id: 0,
            },
            Player {
                color: Cubie::Green,
                id: 1,
            },
        ];

        Self {
            cage: Cage::new(),
            players,
            remaining_cubies: [p1_cubies, p2_cubies],
            player_to_move: players[0],
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

        // TODO: Figure out if we need to be preventing previous move inversions (orig. game rule)
        // Flip
        moves.push(Move::Flip);

        // Rotations
        for layer in [Layer::Down, Layer::Equator, Layer::Up] {
            for rotation in [
                Rotation::Clockwise,
                Rotation::CounterClockwise,
                Rotation::HalfTurn,
            ] {
                let r#move = Move::RotateLayer { layer, rotation };
                moves.push(r#move);
            }
        }

        moves
    }

    pub fn apply_move(&mut self, r#move: Move) -> Result<(), &'static str> {
        match r#move {
            Move::Drop { color, column } => {
                self.cage.drop(color, column)?;
                self.remaining_cubies[self.player_to_move.id as usize] -= 1;
            }
            Move::Flip => {
                self.cage.flip();
            }
            Move::RotateLayer { layer, rotation } => {
                self.cage.rotate_layer(layer, rotation);
            }
        }

        self.player_to_move = if self.player_to_move.id == 0 {
            self.players[1]
        } else {
            self.players[0]
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legal_moves_initial_state() {
        let game = GameState::new(4, 4);
        let legal_moves = game.legal_moves();
        assert!(legal_moves.len() == 18);
    }

    #[test]
    fn test_full_column_drop_illegal() {
        let mut game = GameState::new(4, 4);
        for _ in 0..3 {
            game.cage.drop(game.player_to_move.color, (0, 0)).unwrap();
        }

        let legal_moves = game.legal_moves();
        assert!(legal_moves.len() == 17);
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
}
