use crate::core::{
    cage::Cage,
    cubie::Cubie,
    line::Line,
    r#move::{Layer, Move, Rotation},
    zobrist,
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct Player {
    pub color: Cubie,
    pub id: u8,
}

/// Two-player rules: simultaneous lines and endless play draw; immediate inverse forbidden.
pub const RULES_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum Outcome {
    Win(u8),
    Draw,
}

// TODO: enable more than 2 players and more than 1 color per player
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, Encode, Decode)]
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
        if self.outcome().is_some() {
            return Vec::new();
        }
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

    /// The single transition used by play and search. Rejected moves are atomic.
    pub fn apply_move(&mut self, r#move: Move) -> Result<(), &'static str> {
        if !self.legal_moves().contains(&r#move) {
            return Err("Illegal move");
        }
        match r#move {
            Move::Drop { color, column } => {
                self.cage.drop(color, column)?;
                self.remaining_cubies[self.player_to_move.id as usize] -= 1;
            }
            Move::Flip => self.cage.flip(),
            Move::RotateLayer { layer, rotation } => self.cage.rotate_layer(layer, rotation),
        }
        self.last_move = Some(r#move);
        self.advance_player_to_move();
        self.rebuild_zobrist_hash();
        Ok(())
    }

    /// Infinite play and simultaneous winning lines are draws (rules version 1).
    pub fn outcome(&self) -> Option<Outcome> {
        let mut wins = [false; 2];
        for (color, _) in self.cage.lines() {
            for player in self.players {
                if player.color == color {
                    wins[player.id as usize] = true;
                }
            }
        }
        match wins {
            [true, true] => Some(Outcome::Draw),
            [true, false] => Some(Outcome::Win(0)),
            [false, true] => Some(Outcome::Win(1)),
            _ => None,
        }
    }

    /// A representative winning line for rendering. Draws have no single winner.
    pub fn won(&self) -> Option<(Player, Line)> {
        let Outcome::Win(id) = self.outcome()? else {
            return None;
        };
        let player = self.players[id as usize];
        self.cage
            .lines()
            .find(|(color, _)| *color == player.color)
            .map(|(_, line)| (player, line))
    }

    pub fn normalize(&mut self) {
        *self = self.position_key().to_state();
    }

    pub fn position_key(&self) -> super::position::PositionKey {
        super::position::PositionKey::new(self)
    }

    pub fn apply_move_normalize(&mut self, r#move: Move) -> Result<(), &'static str> {
        self.apply_move(r#move)?;
        self.normalize();
        Ok(())
    }

    /// Validate untrusted imported states before using their indices or move history.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.players[0].id != 0
            || self.players[1].id != 1
            || self.players[0].color == self.players[1].color
            || self.player_to_move.id > 1
            || self.player_to_move != self.players[self.player_to_move.id as usize]
        {
            return Err("Invalid players");
        }
        let mut counts = [0u16; 2];
        for x in 0..3 {
            for y in 0..3 {
                let mut empty = false;
                for z in 0..3 {
                    match self.cage.grid[x][y][z] {
                        None => empty = true,
                        Some(color) => {
                            if Cage::is_center(x, y) || empty {
                                return Err("Invalid cage or gravity");
                            }
                            let Some(id) = self.players.iter().position(|p| p.color == color)
                            else {
                                return Err("Color does not belong to a player");
                            };
                            counts[id] += 1;
                        }
                    }
                }
            }
        }
        if (0..2).any(|i| counts[i] + self.remaining_cubies[i] as u16 > 24) {
            return Err("Invalid inventory");
        }
        match self.last_move {
            Some(Move::Drop {
                color,
                column: (x, y),
            }) if x >= 3
                || y >= 3
                || Cage::is_center(x, y)
                || color != self.players[1 - self.player_to_move.id as usize].color =>
            {
                return Err("Invalid previous drop");
            }
            Some(Move::RotateLayer {
                rotation: Rotation::HalfTurn,
                ..
            }) => {
                return Err("Half turns are not legal moves");
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn rebuild_zobrist_hash(&mut self) {
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
        for color in [Cubie::Blue, Cubie::Red, Cubie::Blue] {
            game.cage.drop(color, (0, 0)).unwrap();
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
