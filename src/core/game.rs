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
    pub id: u8,
}

/// Two players; monochromatic lines win; simultaneous opposing lines and endless play draw.
pub const RULES_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum Outcome {
    Win(u8),
    Draw,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GameState {
    pub cage: Cage,
    pub players: [Player; 2],
    /// Exclusive ownership, indexed by Cubie. Unassigned colors have no pieces.
    pub color_owners: [Option<u8>; 6],
    /// Reserves per exact color, not per player.
    pub remaining_cubies: [u8; 6],
    pub player_to_move: Player,
    pub zobrist_hash: u64,
    pub last_move: Option<Move>,
}

impl GameState {
    pub fn new(p1_cubies: u8, p2_cubies: u8) -> Self {
        let mut owners = [None; 6];
        owners[Cubie::Blue as usize] = Some(0);
        owners[Cubie::Red as usize] = Some(1);
        let mut remaining = [0; 6];
        remaining[Cubie::Blue as usize] = p1_cubies;
        remaining[Cubie::Red as usize] = p2_cubies;
        Self {
            cage: Cage::new(),
            players: [Player { id: 0 }, Player { id: 1 }],
            color_owners: owners,
            remaining_cubies: remaining,
            player_to_move: Player { id: 0 },
            zobrist_hash: 0,
            last_move: None,
        }
    }

    pub fn with_colors(owners: [Option<u8>; 6], remaining: [u8; 6]) -> Result<Self, &'static str> {
        let mut state = Self::new(0, 0);
        state.color_owners = owners;
        state.remaining_cubies = remaining;
        state.validate()?;
        Ok(state)
    }

    pub fn single_color(colors: [Cubie; 2], stocks: [u8; 2]) -> Result<Self, &'static str> {
        if colors[0] == colors[1] {
            return Err("Colors must have distinct owners");
        }
        let mut owners = [None; 6];
        let mut remaining = [0; 6];
        for id in 0..2 {
            owners[colors[id] as usize] = Some(id as u8);
            remaining[colors[id] as usize] = stocks[id];
        }
        Self::with_colors(owners, remaining)
    }

    /// Three distinct colors per player, four pieces of each color.
    pub fn multicolor() -> Self {
        Self::with_colors(
            [Some(0), Some(1), Some(1), Some(1), Some(0), Some(0)],
            [4; 6],
        )
        .unwrap()
    }

    pub fn owner(&self, color: Cubie) -> Option<u8> {
        self.color_owners[color as usize]
    }
    pub fn remaining(&self, color: Cubie) -> u8 {
        self.remaining_cubies[color as usize]
    }
    pub fn colors_for(&self, player: u8) -> impl Iterator<Item = Cubie> + '_ {
        Cubie::ALL
            .into_iter()
            .filter(move |&c| self.owner(c) == Some(player))
    }
    /// None means the specialized solver cannot represent this configuration,
    /// even when its additional assigned colors currently have zero pieces.
    pub fn single_colors(&self) -> Option<[Cubie; 2]> {
        let mut colors = [None; 2];
        for c in Cubie::ALL {
            if let Some(id) = self.owner(c) {
                let slot = colors.get_mut(id as usize)?;
                if slot.replace(c).is_some() {
                    return None;
                }
            }
        }
        Some([colors[0]?, colors[1]?])
    }
    pub fn inventories(&self) -> [u8; 6] {
        let mut totals = self.remaining_cubies;
        for &color in self.cage.grid.iter().flatten().flatten().flatten() {
            totals[color as usize] += 1;
        }
        totals
    }
    pub fn restarted(&self) -> Self {
        let mut initial = *self;
        initial.remaining_cubies = self.inventories();
        initial.cage = Cage::new();
        initial.player_to_move = initial.players[0];
        initial.last_move = None;
        initial.zobrist_hash = 0;
        initial
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.outcome().is_some() {
            return Vec::new();
        }
        let mut moves = Vec::new();
        for color in self
            .colors_for(self.player_to_move.id)
            .filter(|&c| self.remaining(c) > 0)
        {
            for x in 0..3 {
                for y in 0..3 {
                    if !Cage::is_center(x, y) && self.cage.grid[x][y][2].is_none() {
                        moves.push(Move::Drop {
                            color,
                            column: (x, y),
                        });
                    }
                }
            }
        }
        if self.last_move != Some(Move::Flip) {
            moves.push(Move::Flip);
        }
        for layer in [Layer::Down, Layer::Equator, Layer::Up] {
            for rotation in [Rotation::Clockwise, Rotation::CounterClockwise] {
                let m = Move::RotateLayer { layer, rotation };
                if self.last_move != m.inverse() {
                    moves.push(m);
                }
            }
        }
        moves
    }

    /// The single transition used by play and search. Rejected moves are atomic.
    pub fn apply_move(&mut self, m: Move) -> Result<(), &'static str> {
        if !self.legal_moves().contains(&m) {
            return Err("Illegal move");
        }
        match m {
            Move::Drop { color, column } => {
                self.cage.drop(color, column)?;
                self.remaining_cubies[color as usize] -= 1;
            }
            Move::Flip => self.cage.flip(),
            Move::RotateLayer { layer, rotation } => self.cage.rotate_layer(layer, rotation),
        }
        self.last_move = Some(m);
        self.player_to_move = self.players[1 - self.player_to_move.id as usize];
        self.rebuild_zobrist_hash();
        Ok(())
    }

    pub fn outcome(&self) -> Option<Outcome> {
        let mut wins = [false; 2];
        for (color, _) in self.cage.lines() {
            if let Some(owner) = self.owner(color).filter(|&id| id < 2) {
                wins[owner as usize] = true;
            }
        }
        match wins {
            [true, true] => Some(Outcome::Draw),
            [true, false] => Some(Outcome::Win(0)),
            [false, true] => Some(Outcome::Win(1)),
            _ => None,
        }
    }

    /// A representative winning color and line. Draws have no single winner.
    pub fn won(&self) -> Option<(Player, Cubie, Line)> {
        let Outcome::Win(id) = self.outcome()? else {
            return None;
        };
        self.cage
            .lines()
            .find(|(color, _)| self.owner(*color) == Some(id))
            .map(|(color, line)| (self.players[id as usize], color, line))
    }
    pub fn normalize(&mut self) {
        *self = self.position_key().to_state();
    }
    pub fn position_key(&self) -> super::position::PositionKey {
        super::position::PositionKey::new(self)
    }
    pub fn apply_move_normalize(&mut self, m: Move) -> Result<(), &'static str> {
        self.apply_move(m)?;
        self.normalize();
        Ok(())
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.players != [Player { id: 0 }, Player { id: 1 }]
            || self.player_to_move.id > 1
            || self.color_owners.iter().flatten().any(|&id| id > 1)
        {
            return Err("Invalid players or color ownership");
        }
        let mut counts = self.remaining_cubies.map(u16::from);
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
                            counts[color as usize] += 1;
                        }
                    }
                }
            }
        }
        let mut totals = [0u16; 2];
        for color in Cubie::ALL {
            match self.owner(color) {
                Some(id) => totals[id as usize] += counts[color as usize],
                None if counts[color as usize] != 0 => {
                    return Err("Color does not belong to a player");
                }
                None => (),
            }
        }
        if totals.iter().any(|&n| n > 24) {
            return Err("A player cannot have more than 24 cubies");
        }
        match self.last_move {
            Some(Move::Drop {
                color,
                column: (x, y),
            }) if x >= 3
                || y >= 3
                || Cage::is_center(x, y)
                || self.owner(color) != Some(1 - self.player_to_move.id) =>
            {
                return Err("Invalid previous drop");
            }
            Some(Move::RotateLayer {
                rotation: Rotation::HalfTurn,
                ..
            }) => return Err("Half turns are not legal moves"),
            _ => (),
        }
        Ok(())
    }
    pub(crate) fn rebuild_zobrist_hash(&mut self) {
        self.zobrist_hash = 0;
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    if let Some(c) = self.cage.grid[x][y][z] {
                        self.zobrist_hash ^= zobrist::POS_COLOR[c as usize][x][y][z];
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
            color: game.single_colors().unwrap()[game.player_to_move.id as usize],
            column: (0, 0)
        }));
    }

    #[test]
    fn test_out_of_turn_drop_illegal() {
        let mut game = GameState::new(4, 4);
        game.apply_move(Move::Drop {
            color: Cubie::Blue,
            column: (1, 2),
        })
        .unwrap();

        // Now it's player 1's turn, so player 0 cannot drop a cubie
        assert!(!game.legal_moves().contains(&Move::Drop {
            color: Cubie::Blue,
            column: (1, 2)
        }));
        // Player 1 can
        assert!(game.legal_moves().contains(&Move::Drop {
            color: Cubie::Red,
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
            color: game.single_colors().unwrap()[game.player_to_move.id as usize],
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
            color: game.single_colors().unwrap()[game.player_to_move.id as usize],
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
                color: game1.single_colors().unwrap()[game1.player_to_move.id as usize],
                column: (0, 0),
            })
            .unwrap();

        let mut game2 = GameState::new(2, 2);
        game2
            .apply_move(Move::Drop {
                color: game2.single_colors().unwrap()[game2.player_to_move.id as usize],
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
