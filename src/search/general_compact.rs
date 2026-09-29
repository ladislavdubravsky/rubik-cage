//! Search-only 76-bit positions: 24 three-bit cells, turn and inverse restriction.
//! Ownership and exact color totals belong to the checked namespace. Public
//! results still use complete PositionKeys. Color permutations are allowed only
//! within equal-owner, equal-total classes; board counts then determine reserves.
use super::Evaluation;
use crate::core::{
    cubie::Cubie,
    game::GameState,
    line::LINES,
    r#move::{Layer, Move, Rotation},
};

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
const BOARD: u128 = (1 << 72) - 1;
const LOW_BITS: u128 = BOARD / 7;
const ROW: u128 = (1 << 24) - 1;
const fn reverse_four() -> [u16; 4096] {
    let mut table = [0; 4096];
    let mut i = 0;
    while i < 4096 {
        let mut cell = 0;
        while cell < 4 {
            table[i] |= (((i >> (3 * cell)) & 7) << (3 * (3 - cell))) as u16;
            cell += 1;
        }
        i += 1;
    }
    table
}
const REVERSE_FOUR: [u16; 4096] = reverse_four();
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position(pub(crate) u128);
impl Position {
    pub fn turn(self) -> u8 {
        ((self.0 >> 72) & 1) as u8
    }
    fn ban(self) -> u8 {
        (self.0 >> 73) as u8
    }
    fn cell(self, i: usize) -> u8 {
        ((self.0 >> (3 * i)) & 7) as u8
    }
}
#[derive(Clone, Debug)]
pub struct Space {
    pub(crate) owners: [Option<u8>; 6],
    pub(crate) totals: [u8; 6],
    groups: Vec<Vec<u8>>,
    pub(crate) relabel: bool,
    pub(crate) ordering: bool,
}
const fn lines() -> [[usize; 3]; 28] {
    let mut out = [[0; 3]; 28];
    let mut l = 0;
    while l < 28 {
        let mut j = 0;
        while j < 3 {
            let [x, y, z] = LINES[l][j];
            let mut c = 0;
            while RING[c].0 != x || RING[c].1 != y {
                c += 1;
            }
            out[l][j] = z * 8 + c;
            j += 1;
        }
        l += 1;
    }
    out
}
const LINE_CELLS: [[usize; 3]; 28] = lines();
impl Space {
    pub fn new(state: &GameState) -> Result<Self, &'static str> {
        state.validate()?;
        let owners = state.color_owners;
        let totals = state.inventories();
        let mut groups: Vec<Vec<u8>> = Vec::new();
        for c in 0..6 {
            if let Some(group) = groups.iter_mut().find(|g| {
                let other = usize::from(g[0] - 1);
                owners[c] == owners[other] && totals[c] == totals[other]
            }) {
                group.push(c as u8 + 1);
            } else {
                groups.push(vec![c as u8 + 1]);
            }
        }
        Ok(Self {
            owners,
            totals,
            groups,
            relabel: true,
            ordering: true,
        })
    }
    pub fn matches(&self, state: &GameState) -> bool {
        self.owners == state.color_owners && self.totals == state.inventories()
    }
    pub fn try_encode(&self, state: &GameState) -> Result<Position, &'static str> {
        state.validate()?;
        if !self.matches(state) {
            return Err("Compact color space mismatch");
        }
        Ok(self.encode(state))
    }
    pub(crate) fn encode(&self, state: &GameState) -> Position {
        let mut bits = 0;
        for (c, &(x, y)) in RING.iter().enumerate() {
            for z in 0..3 {
                if let Some(color) = state.cage.grid[x][y][z] {
                    bits |= (color as u128 + 1) << (3 * (z * 8 + c));
                }
            }
        }
        let ban = match state.last_move.and_then(Move::inverse) {
            None => 0,
            Some(Move::Flip) => 1,
            Some(Move::RotateLayer { layer, rotation }) => {
                2 + 2 * layer as u128 + u128::from(rotation == Rotation::CounterClockwise)
            }
            _ => unreachable!(),
        };
        self.canonical(Position(
            bits | (u128::from(state.player_to_move.id) << 72) | (ban << 73),
        ))
    }
    pub fn decode(&self, pos: Position) -> GameState {
        let mut state = GameState::with_colors(self.owners, self.totals).unwrap();
        state.player_to_move = state.players[pos.turn() as usize];
        for i in 0..24 {
            let code = pos.cell(i);
            if code != 0 {
                let (x, y) = RING[i % 8];
                state.cage.grid[x][y][i / 8] = Some(Cubie::ALL[code as usize - 1]);
                state.remaining_cubies[code as usize - 1] -= 1;
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
        state.rebuild_zobrist_hash();
        state
    }
    pub fn canonical(&self, pos: Position) -> Position {
        let mut best = 0;
        let relabel = self.relabel && self.groups.iter().any(|g| g.len() > 1);
        let board = pos.0 & BOARD;
        let mut reflected = 0;
        for z in 0..3 {
            let row = (board >> (24 * z)) & ROW;
            let reversed = u128::from(REVERSE_FOUR[(row & 4095) as usize]) << 12
                | u128::from(REVERSE_FOUR[(row >> 12) as usize]);
            // c -> 7-c, then +3 gives the core reflection c -> 2-c.
            reflected |= (((reversed << 9) | (reversed >> 15)) & ROW) << (24 * z);
        }
        for s in 0..8 {
            let source = if s >= 4 { reflected } else { board };
            let shift = (s % 4) * 6;
            let mut transformed = 0;
            for z in 0..3 {
                let row = (source >> (24 * z)) & ROW;
                transformed |= (((row << shift) | (row >> (24 - shift))) & ROW) << (24 * z);
            }
            let mut bits = transformed;
            if relabel {
                bits = 0;
                // A color's sparse mask orders it by its first occupied cell.
                // Sort masks only within equal-owner/equal-total classes. This
                // implements first-occurrence relabeling without scanning all
                // 24 cells or enumerating any color permutations.
                for group in &self.groups {
                    let mut masks = [0u128; 6];
                    for (i, &code) in group.iter().enumerate() {
                        let x = transformed ^ (LOW_BITS * u128::from(code));
                        masks[i] = !(x | (x >> 1) | (x >> 2)) & LOW_BITS;
                    }
                    masks[..group.len()].sort_unstable();
                    for (i, &label) in group.iter().enumerate() {
                        bits |= masks[i] * u128::from(label);
                    }
                }
            }

            let ban = if s >= 4 && pos.ban() >= 2 {
                pos.ban() ^ 1
            } else {
                pos.ban()
            };
            bits |= u128::from(pos.turn()) << 72 | u128::from(ban) << 73;
            best = best.max(bits);
        }
        Position(best)
    }
    pub fn terminal(&self, pos: Position) -> Option<Evaluation> {
        let mut winners = 0;
        for [a, b, c] in LINE_CELLS {
            let color = pos.cell(a);
            if color != 0 && color == pos.cell(b) && color == pos.cell(c) {
                winners |= 1 << self.owners[color as usize - 1].unwrap();
            }
        }
        match winners {
            0 => None,
            1 => Some(Evaluation::Win {
                winner: 0,
                plies: 0,
            }),
            2 => Some(Evaluation::Win {
                winner: 1,
                plies: 0,
            }),
            _ => Some(Evaluation::Draw),
        }
    }
    fn counts(&self, pos: Position) -> [u8; 6] {
        let mut counts = [0; 6];
        for i in 0..24 {
            let c = pos.cell(i);
            if c != 0 {
                counts[c as usize - 1] += 1;
            }
        }
        counts
    }
    pub fn earliest(&self, pos: Position, target: u8) -> Option<u16> {
        let counts = self.counts(pos);
        (0..6)
            .filter(|&c| self.owners[c] == Some(target) && self.totals[c] >= 3)
            .map(|c| {
                let missing = 3u16.saturating_sub(u16::from(counts[c]));
                if missing == 0 {
                    1
                } else {
                    2 * missing - u16::from(pos.turn() == target)
                }
            })
            .min()
    }
    fn score(&self, pos: Position, player: u8) -> i32 {
        if let Some(value) = self.terminal(pos) {
            return match value.winner() {
                Some(w) if w == player => 100_000,
                Some(_) => -100_000,
                None => 0,
            };
        }
        let mut score = 0;
        for line in LINE_CELLS {
            let cells = line.map(|i| pos.cell(i));
            let color = cells.iter().copied().find(|&c| c != 0).unwrap_or(0);
            // Other friendly colors block a monochromatic line too.
            if color == 0 || cells.iter().any(|&c| c != 0 && c != color) {
                continue;
            }
            if self.totals[color as usize - 1] < 3 {
                continue;
            }
            let count = cells.iter().filter(|&&c| c != 0).count();
            score += if self.owners[color as usize - 1] == Some(player) {
                [0, 1, 15, 1000][count]
            } else {
                -[0, 1, 20, 1000][count]
            };
        }
        score
    }
    /// Antisymmetric positional potential for the approximate player. This is
    /// deliberately separate from the asymmetric proof-search move ordering.
    pub(crate) fn heuristic(&self, pos: Position, player: u8) -> i32 {
        let counts = self.counts(pos);
        let mut score = 0;
        for line in LINE_CELLS {
            let cells = line.map(|i| pos.cell(i));
            let color = cells.iter().copied().find(|&c| c != 0).unwrap_or(0);
            if color == 0 || cells.iter().any(|&c| c != 0 && c != color) {
                continue;
            }
            let c = color as usize - 1;
            if self.totals[c] < 3 {
                continue;
            }
            let count = cells.iter().filter(|&&c| c != 0).count();
            let reserve = self.totals[c] - counts[c];
            let support: usize = line
                .iter()
                .filter(|&&i| pos.cell(i) == 0)
                .map(|&i| (0..i / 8).filter(|&z| pos.cell(z * 8 + i % 8) == 0).count())
                .sum();
            let value = match count {
                2 if reserve > 0 && support == 0 => 1200,
                2 => 300,
                1 if reserve >= 2 => 100 / (1 + support as i32),
                1 => 25 / (1 + support as i32),
                _ => 0,
            };
            score += if self.owners[c] == Some(player) {
                value
            } else {
                -value
            };
        }
        score.clamp(-100_000, 100_000)
    }
    pub(crate) fn volatile(&self, pos: Position) -> bool {
        LINE_CELLS.iter().any(|line| {
            let cells = line.map(|i| pos.cell(i));
            let color = cells.iter().copied().find(|&c| c != 0).unwrap_or(0);
            color != 0
                && self.totals[color as usize - 1] >= 3
                && cells.iter().filter(|&&c| c == color).count() == 2
                && cells.iter().all(|&c| c == 0 || c == color)
        })
    }
    pub fn children(&self, pos: Position) -> Vec<Position> {
        if self.terminal(pos).is_some() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(31);
        let counts = self.counts(pos);
        let next = u128::from(1 - pos.turn()) << 72;
        let board = pos.0 & BOARD;
        for (color, &count) in counts.iter().enumerate() {
            if self.owners[color] != Some(pos.turn()) || count >= self.totals[color] {
                continue;
            }
            for c in 0..8 {
                if let Some(z) = (0..3).find(|&z| pos.cell(z * 8 + c) == 0) {
                    out.push(self.canonical(Position(
                        board | ((color as u128 + 1) << (3 * (z * 8 + c))) | next,
                    )));
                }
            }
        }
        if pos.ban() != 1 {
            let mut bits = 0;
            for i in 0..24 {
                let dest = (2 - i / 8) * 8 + (14 - i % 8) % 8;
                bits |= u128::from(pos.cell(i)) << (3 * dest);
            }
            out.push(self.canonical(Position(gravity(bits) | next | (1 << 73))));
        }
        for layer in 0..3 {
            for direction in 0..2 {
                let ban = 2 + 2 * layer + direction;
                if usize::from(pos.ban()) == ban {
                    continue;
                }
                let shift = layer * 24;
                let row = (board >> shift) & ROW;
                let rotated = if direction == 0 {
                    ((row << 6) | (row >> 18)) & ROW
                } else {
                    ((row >> 6) | (row << 18)) & ROW
                };
                let bits = (board & !(ROW << shift)) | (rotated << shift);
                out.push(
                    self.canonical(Position(gravity(bits) | next | (((ban ^ 1) as u128) << 73))),
                );
            }
        }
        out.sort_unstable();
        out.dedup();
        if self.ordering {
            out.sort_by_cached_key(|&p| std::cmp::Reverse(self.score(p, pos.turn())));
        } else {
            out.sort_by_key(|&p| match self.terminal(p) {
                Some(Evaluation::Win { winner, .. }) if winner == pos.turn() => 0,
                Some(_) => 1,
                None => 2,
            });
        }
        out
    }
}
fn gravity(bits: u128) -> u128 {
    let mut result = 0;
    for col in 0..8 {
        let mut height = 0;
        for z in 0..3 {
            let code = (bits >> (3 * (z * 8 + col))) & 7;
            if code != 0 {
                result |= code << (3 * (height * 8 + col));
                height += 1;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    fn check(state: GameState) {
        for relabel in [false, true] {
            let mut space = Space::new(&state).unwrap();
            space.relabel = relabel;
            let pos = space.try_encode(&state).unwrap();
            assert_eq!(space.terminal(pos), Evaluation::terminal(&state));
            let decoded = space.decode(pos);
            decoded.validate().unwrap();
            assert_eq!(space.try_encode(&decoded).unwrap(), pos);
            if !relabel {
                assert_eq!(decoded.position_key(), state.position_key());
            }
            let expected: HashSet<_> = state
                .legal_moves()
                .into_iter()
                .map(|m| {
                    let mut next = state;
                    next.apply_move(m).unwrap();
                    space.try_encode(&next).unwrap()
                })
                .collect();
            let actual: HashSet<_> = space.children(pos).into_iter().collect();
            assert_eq!(actual, expected, "{state:?}");
            for child in actual {
                space.decode(child).validate().unwrap();
            }
        }
    }
    #[test]
    fn compact_transitions_match_core_through_full_games_and_all_restrictions() {
        let configs = [
            GameState::multicolor(),
            GameState::with_colors([Some(0); 6], [3; 6]).unwrap(),
            GameState::with_colors(GameState::multicolor().color_owners, [1, 2, 3, 4, 5, 6])
                .unwrap(),
            GameState::new(3, 1),
        ];
        let mut rng = 0x12345678u64;
        for root in configs {
            for _ in 0..20 {
                let mut state = root;
                for _ in 0..32 {
                    check(state);
                    let moves = state.legal_moves();
                    if moves.is_empty() {
                        break;
                    }
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    state
                        .apply_move(moves[(rng >> 32) as usize % moves.len()])
                        .unwrap();
                }
            }
        }
    }
    #[test]
    fn complete_small_graph_and_both_owner_terminals_match_core() {
        let root = GameState::with_colors(GameState::multicolor().color_owners, [3, 1, 0, 0, 0, 0])
            .unwrap();
        let graph =
            crate::search::retrograde::solve(&root, &Default::default(), Default::default())
                .unwrap();
        for key in graph.values.keys() {
            check(key.to_state());
        }
        for pair in [(Cubie::White, Cubie::Blue), (Cubie::White, Cubie::Red)] {
            let mut state = GameState::multicolor();
            for (y, color) in [(0, pair.0), (2, pair.1)] {
                for x in 0..3 {
                    state.cage.drop(color, (x, y)).unwrap();
                    state.remaining_cubies[color as usize] -= 1;
                }
            }
            check(state);
        }
    }
    fn permute(mut state: GameState, map: [usize; 6]) -> GameState {
        for c in state.cage.grid.iter_mut().flatten().flatten().flatten() {
            *c = Cubie::ALL[map[*c as usize]];
        }
        let remaining = state.remaining_cubies;
        for c in 0..6 {
            state.remaining_cubies[map[c]] = remaining[c];
        }
        if let Some(Move::Drop { color, column }) = state.last_move {
            state.last_move = Some(Move::Drop {
                color: Cubie::ALL[map[color as usize]],
                column,
            });
        }
        state
    }
    #[test]
    fn color_relabeling_is_exact_only_with_equal_owner_and_total() {
        let mut state = GameState::multicolor();
        for (color, column) in [
            (Cubie::White, (0, 0)),
            (Cubie::Yellow, (1, 0)),
            (Cubie::Blue, (2, 2)),
            (Cubie::Red, (0, 2)),
        ] {
            state.apply_move(Move::Drop { color, column }).unwrap();
        }
        let space = Space::new(&state).unwrap();
        let a = space.try_encode(&state).unwrap();
        for p in [
            [0, 4, 5],
            [0, 5, 4],
            [4, 0, 5],
            [4, 5, 0],
            [5, 0, 4],
            [5, 4, 0],
        ] {
            for q in [
                [1, 2, 3],
                [1, 3, 2],
                [2, 1, 3],
                [2, 3, 1],
                [3, 1, 2],
                [3, 2, 1],
            ] {
                let mut map = [0; 6];
                for (i, c) in [0, 4, 5].into_iter().enumerate() {
                    map[c] = p[i];
                }
                for (i, c) in [1, 2, 3].into_iter().enumerate() {
                    map[c] = q[i];
                }
                let other = permute(state, map);
                assert_eq!(a, space.try_encode(&other).unwrap());
                check(other);
            }
        }
        state.remaining_cubies[0] += 1;
        let unequal = Space::new(&state).unwrap();
        let swapped = permute(state, [4, 1, 2, 3, 0, 5]);
        assert!(unequal.try_encode(&swapped).is_err());
        let mut wrong_owner = state;
        wrong_owner.color_owners.swap(0, 1);
        assert!(unequal.try_encode(&wrong_owner).is_err());
        // Mixed colors of one owner must remain separate even when interchangeable.
        let decoded = space.decode(a);
        assert_eq!(
            decoded
                .cage
                .grid
                .iter()
                .flatten()
                .flatten()
                .filter(|c| c.is_some())
                .count(),
            4
        );
        assert_eq!(space.terminal(a), None);
    }
}
