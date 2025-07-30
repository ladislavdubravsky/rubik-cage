use crate::{cage::Cage, cubie::Cubie, r#move::{Layer, Move, Rotation}};
use std::collections::HashMap;

type Player = Cubie;

// TODO: enable more than 2 players and more than 1 color per player
pub struct GameState {
    pub cage: Cage,
    pub players: [Player; 2],
    pub remaining_cubies: HashMap<Player, u8>,
    pub player_to_move: Player,
    pub last_move: Option<Move>,
}

impl GameState {
    pub fn new(players: [Player; 2], cubies: u8) -> Self {
        let mut remaining_cubies = HashMap::new();
        for &player in &players {
            remaining_cubies.insert(player, cubies);
        }

        Self {
            cage: Cage::new(),
            players,
            remaining_cubies,
            player_to_move: players[0],
            last_move: None,
        }
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();

        // Drops into non-full columns by the player to move. Allowed if the player still has
        // cubies to drop.
        if self.remaining_cubies[&self.player_to_move] > 0 {
            for x in 0..3 {
                for y in 0..3 {
                    if Cage::is_center(x, y) {
                        continue;
                    }
                    if self.cage.grid[x][y][2].is_none() {
                        moves.push(Move::Drop {
                            color: self.player_to_move,
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
            for rotation in [Rotation::Clockwise, Rotation::CounterClockwise, Rotation::HalfTurn] {
                let r#move = Move::RotateLayer { layer, rotation };
                if self.last_move != r#move.inverse() {
                    moves.push(r#move);
                }
            }
        }

        moves
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cubie::Cubie;

    #[test]
    fn test_legal_moves_initial_state() {
        let game_state = GameState::new([Player::Red, Player::Green], 4);
        let legal_moves = game_state.legal_moves();
        assert!(legal_moves.len() == 18);
    }
}
