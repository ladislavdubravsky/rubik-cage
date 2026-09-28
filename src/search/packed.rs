//! Compact search-only positions. Inventories/colors are fixed by `Space`, so a
//! 52-bit key is exact within that space. Public/persistent evaluation keys remain
//! complete PositionKeys, never these context-dependent integers.
use crate::core::{
    cubie::Cubie,
    game::{GameState, Player},
    line::LINES,
    r#move::{Layer, Move, Rotation},
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub const MASK: u64 = (1 << 24) - 1;
pub const POSITION_MASK: u64 = (1 << 52) - 1;
const RING: [(usize, usize); 8] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (2, 1),
    (2, 2),
    (1, 2),
    (0, 2),
    (0, 1),
];

const fn transformations() -> [[u8; 256]; 8] {
    let mut table = [[0; 256]; 8];
    let mut s = 0;
    while s < 8 {
        let mut byte = 0;
        while byte < 256 {
            let mut i = 0;
            while i < 8 {
                let j = if s >= 4 {
                    (10 - i + 2 * (s % 4)) % 8
                } else {
                    (i + 2 * s) % 8
                };
                if byte & (1 << i) != 0 {
                    table[s][byte] |= 1 << j;
                }
                i += 1;
            }
            byte += 1;
        }
        s += 1;
    }
    table
}
const TRANSFORM: [[u8; 256]; 8] = transformations();

fn line_masks() -> [u32; 28] {
    LINES.map(|line| {
        line.into_iter().fold(0, |mask, [x, y, z]| {
            let column = RING.iter().position(|p| *p == (x, y)).unwrap();
            mask | (1 << (8 * z + column))
        })
    })
}
static LINES_BITS: std::sync::LazyLock<[u32; 28]> = std::sync::LazyLock::new(line_masks);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct Space {
    pub totals: [u8; 2],
    pub colors: [Cubie; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Position(pub u64);

impl Space {
    pub fn new(state: &GameState) -> Result<Self, &'static str> {
        state.validate()?;
        let mut totals = state.remaining_cubies;
        for c in state.cage.grid.iter().flatten().flatten().flatten() {
            totals[state.players.iter().position(|p| p.color == *c).unwrap()] += 1;
        }
        Ok(Self {
            totals,
            colors: state.players.map(|p| p.color),
        })
    }
    pub fn encode(&self, state: &GameState) -> Position {
        assert_eq!(Self::new(state).unwrap(), *self, "Search space mismatch");
        self.encode_board(state)
    }
    pub(crate) fn encode_board(&self, state: &GameState) -> Position {
        let mut bits = 0;
        for (column, &(x, y)) in RING.iter().enumerate() {
            for z in 0..3 {
                if let Some(c) = state.cage.grid[x][y][z] {
                    let owner = self.colors.iter().position(|color| *color == c).unwrap();
                    bits |= 1 << (owner * 24 + z * 8 + column);
                }
            }
        }
        let ban = match state.last_move.and_then(Move::inverse) {
            None => 0,
            Some(Move::Flip) => 1,
            Some(Move::RotateLayer { layer, rotation }) => {
                2 + 2 * layer as u64 + u64::from(rotation == Rotation::CounterClockwise)
            }
            _ => unreachable!(),
        };
        Position(bits | (u64::from(state.player_to_move.id) << 48) | (ban << 49)).canonical()
    }
    pub fn decode(&self, pos: Position) -> GameState {
        let mut state = GameState::new(self.totals[0], self.totals[1]);
        state.players = [
            Player {
                id: 0,
                color: self.colors[0],
            },
            Player {
                id: 1,
                color: self.colors[1],
            },
        ];
        state.player_to_move = state.players[pos.turn() as usize];
        for owner in 0..2 {
            let bits = pos.board(owner);
            state.remaining_cubies[owner as usize] -= bits.count_ones() as u8;
            for (column, &(x, y)) in RING.iter().enumerate() {
                for z in 0..3 {
                    if bits & (1 << (z * 8 + column)) != 0 {
                        state.cage.grid[x][y][z] = Some(self.colors[owner as usize]);
                    }
                }
            }
        }
        state.last_move = match pos.ban() {
            0 => None,
            1 => Some(Move::Flip),
            b => Some(Move::RotateLayer {
                layer: [Layer::Down, Layer::Equator, Layer::Up][((b - 2) / 2) as usize],
                rotation: if b % 2 == 0 {
                    Rotation::CounterClockwise
                } else {
                    Rotation::Clockwise
                },
            }),
        };
        state
    }
    /// A necessary (not sufficient) earliest possible target victory.
    pub fn earliest(&self, pos: Position, target: u8) -> u8 {
        if self.totals[target as usize] < 3 {
            return u8::MAX;
        }
        let missing = 3u8.saturating_sub(pos.board(target).count_ones() as u8);
        if missing == 0 {
            1
        } else {
            2 * missing - u8::from(pos.turn() == target)
        }
    }
    pub fn children(&self, pos: Position) -> Children {
        let mut children = Children {
            items: [Position(0); 15],
            len: 0,
        };
        if pos.terminal().is_some() {
            return children;
        }
        let owner = pos.turn();
        let board = pos.0 & ((1 << 48) - 1);
        let next_turn = u64::from(1 - owner) << 48;
        let occupancy = pos.board(0) | pos.board(1);
        if pos.board(owner).count_ones() < u32::from(self.totals[owner as usize]) {
            for col in 0..8 {
                if occupancy & (1 << (16 + col)) == 0 {
                    let z = if occupancy & (1 << col) == 0 {
                        0
                    } else if occupancy & (1 << (8 + col)) == 0 {
                        1
                    } else {
                        2
                    };
                    children.push(Position(
                        board | (1 << (u64::from(owner) * 24 + z * 8 + col)) | next_turn,
                    ));
                }
            }
        }
        if pos.ban() != 1 {
            let mut flipped = 0;
            for owner in 0..2 {
                let bits = pos.board(owner);
                for z in 0..3 {
                    let row = ((bits >> (z * 8)) & 255) as usize;
                    // y -> 2-y: ring i -> 6-i, which is reflection + half turn.
                    flipped |= u64::from(TRANSFORM[6][row]) << (owner * 24 + (2 - z) * 8);
                }
            }
            children.push(Position(gravity(flipped) | next_turn | (1 << 49)));
        }
        for layer in 0..3 {
            for direction in 0..2 {
                let m = 2 + 2 * layer + direction;
                if pos.ban() == m {
                    continue;
                }
                let mut rotated = board;
                for owner in 0..2 {
                    let shift = owner * 24 + layer * 8;
                    let row = ((board >> shift) & 255) as u8;
                    let row = if direction == 0 {
                        row.rotate_left(2)
                    } else {
                        row.rotate_right(2)
                    };
                    rotated = (rotated & !(255 << shift)) | (u64::from(row) << shift);
                }
                let inverse = 2 + 2 * layer + (1 - direction);
                children.push(Position(gravity(rotated) | next_turn | (inverse << 49)));
            }
        }
        children
    }
}

fn gravity(board: u64) -> u64 {
    let mut result = 0;
    for col in 0..8 {
        let mut height = 0;
        for z in 0..3 {
            let bit = 1 << (z * 8 + col);
            if board & bit != 0 {
                result |= 1 << (height * 8 + col);
                height += 1;
            } else if (board >> 24) & bit != 0 {
                result |= 1 << (24 + height * 8 + col);
                height += 1;
            }
        }
    }
    result
}

impl Position {
    pub fn board(self, owner: u8) -> u32 {
        ((self.0 >> (owner * 24)) & MASK) as u32
    }
    pub fn turn(self) -> u8 {
        ((self.0 >> 48) & 1) as u8
    }
    fn ban(self) -> u64 {
        self.0 >> 49
    }
    pub fn canonical(self) -> Self {
        let mut best = 0;
        for (s, table) in TRANSFORM.iter().enumerate() {
            let mut transformed = 0;
            for byte in 0..6 {
                transformed |=
                    u64::from(table[((self.0 >> (8 * byte)) & 255) as usize]) << (8 * byte);
            }
            let ban = if s >= 4 && self.ban() >= 2 {
                self.ban() ^ 1
            } else {
                self.ban()
            };
            transformed |= (u64::from(self.turn()) << 48) | (ban << 49);
            best = best.max(transformed);
        }
        Self(best)
    }
    /// -1 = terminal draw, 0/1 = winning player, None = ongoing.
    pub fn terminal(self) -> Option<i8> {
        let a = self.board(0);
        let b = self.board(1);
        let aw = a.count_ones() >= 3 && LINES_BITS.iter().any(|&line| a & line == line);
        let bw = b.count_ones() >= 3 && LINES_BITS.iter().any(|&line| b & line == line);
        match (aw, bw) {
            (true, true) => Some(-1),
            (true, false) => Some(0),
            (false, true) => Some(1),
            _ => None,
        }
    }
    pub fn ordering(self, player: u8) -> i32 {
        if let Some(winner) = self.terminal() {
            return if winner == player as i8 {
                10000
            } else {
                -10000
            };
        }
        let own = self.board(player);
        let other = self.board(1 - player);
        let mut score = own.count_ones() as i32;
        for &line in LINES_BITS.iter() {
            if other & line == 0 {
                score += [0, 1, 15, 1000][(own & line).count_ones() as usize];
            }
            if own & line == 0 {
                score -= [0, 1, 20, 1000][(other & line).count_ones() as usize];
            }
        }
        score
    }
}

pub struct Children {
    pub items: [Position; 15],
    pub len: usize,
}
impl Children {
    fn push(&mut self, pos: Position) {
        let pos = pos.canonical();
        if !self.items[..self.len].contains(&pos) {
            self.items[self.len] = pos;
            self.len += 1;
        }
    }
    pub fn ordered(&mut self, owner: u8) -> &[Position] {
        let mut scores = [0; 15];
        for i in 0..self.len {
            scores[i] = self.items[i].ordering(owner);
            let mut j = i;
            while j > 0 && scores[j] > scores[j - 1] {
                scores.swap(j, j - 1);
                self.items.swap(j, j - 1);
                j -= 1;
            }
        }
        &self.items[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{EvaluationMap, retrograde};
    use std::collections::HashSet;
    #[test]
    fn compact_transitions_match_every_small_game_state() {
        for (a, b) in [(1, 1), (3, 0), (3, 1), (3, 2)] {
            let initial = GameState::new(a, b);
            let space = Space::new(&initial).unwrap();
            let solved =
                retrograde::solve(&initial, &EvaluationMap::new(), Default::default()).unwrap();
            for key in solved.values.keys() {
                let state = key.to_state();
                let packed = space.encode(&state);
                assert_eq!(space.decode(packed).position_key(), *key);
                assert_eq!(packed.canonical(), packed);
                let expected: HashSet<_> = state
                    .legal_moves()
                    .into_iter()
                    .map(|m| {
                        let mut child = state;
                        child.apply_move(m).unwrap();
                        space.encode(&child)
                    })
                    .collect();
                let children = space.children(packed);
                assert_eq!(
                    expected,
                    children.items[..children.len].iter().copied().collect()
                );
                assert_eq!(
                    packed.terminal(),
                    state.outcome().map(|o| match o {
                        crate::core::game::Outcome::Draw => -1,
                        crate::core::game::Outcome::Win(w) => w as i8,
                    })
                );
            }
        }
    }
}
