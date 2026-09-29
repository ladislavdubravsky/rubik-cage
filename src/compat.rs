//! Frozen single-color wire layouts. Do not replace fields with live model types:
//! enum codes, field order and integer widths are part of the existing file formats.
//! New formats belong in separate types, with explicit validated conversions.
use crate::core::{
    cage::Cage,
    cubie::Cubie,
    game::{GameState, Player},
    r#move::{Layer, Move, Rotation},
    position::PositionKey,
};
use bincode::{Decode, Encode};

pub(crate) const SINGLE_COLOR_RULES: u32 = 1;

// Keep this enum small in memory as well as stable on disk: table readers hold
// hundreds of thousands of these boards. Option<u32> would inflate every cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(crate) enum ColorV1 {
    White,
    Yellow,
    Red,
    Orange,
    Blue,
    Green,
}

fn color_code(color: Cubie) -> ColorV1 {
    match color {
        Cubie::White => ColorV1::White,
        Cubie::Yellow => ColorV1::Yellow,
        Cubie::Red => ColorV1::Red,
        Cubie::Orange => ColorV1::Orange,
        Cubie::Blue => ColorV1::Blue,
        Cubie::Green => ColorV1::Green,
    }
}
fn color(code: ColorV1) -> Cubie {
    match code {
        ColorV1::White => Cubie::White,
        ColorV1::Yellow => Cubie::Yellow,
        ColorV1::Red => Cubie::Red,
        ColorV1::Orange => Cubie::Orange,
        ColorV1::Blue => Cubie::Blue,
        ColorV1::Green => Cubie::Green,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(crate) struct PlayerV1 {
    pub color: ColorV1,
    pub id: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(crate) enum MoveV1 {
    Drop {
        color: ColorV1,
        column: (usize, usize),
    },
    RotateLayer {
        layer: u32,
        rotation: u32,
    },
    Flip,
}
impl From<Move> for MoveV1 {
    fn from(m: Move) -> Self {
        match m {
            Move::Drop { color, column } => Self::Drop {
                color: color_code(color),
                column,
            },
            Move::RotateLayer { layer, rotation } => Self::RotateLayer {
                layer: match layer {
                    Layer::Down => 0,
                    Layer::Equator => 1,
                    Layer::Up => 2,
                },
                rotation: match rotation {
                    Rotation::Clockwise => 0,
                    Rotation::CounterClockwise => 1,
                    Rotation::HalfTurn => 2,
                },
            },
            Move::Flip => Self::Flip,
        }
    }
}
impl MoveV1 {
    pub(crate) fn into_current(self) -> Result<Move, &'static str> {
        Ok(match self {
            Self::Drop { color: c, column } => Move::Drop {
                color: color(c),
                column,
            },
            Self::RotateLayer { layer, rotation } => Move::RotateLayer {
                layer: match layer {
                    0 => Layer::Down,
                    1 => Layer::Equator,
                    2 => Layer::Up,
                    _ => return Err("Invalid legacy layer"),
                },
                rotation: match rotation {
                    0 => Rotation::Clockwise,
                    1 => Rotation::CounterClockwise,
                    2 => Rotation::HalfTurn,
                    _ => return Err("Invalid legacy rotation"),
                },
            },
            Self::Flip => Move::Flip,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(crate) struct CageV1([[[Option<ColorV1>; 3]; 3]; 3]);
impl From<Cage> for CageV1 {
    fn from(cage: Cage) -> Self {
        Self(
            cage.grid
                .map(|plane| plane.map(|column| column.map(|c| c.map(color_code)))),
        )
    }
}
impl CageV1 {
    pub(crate) fn into_current(self) -> Result<Cage, &'static str> {
        let mut cage = Cage::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    cage.grid[x][y][z] = self.0[x][y][z].map(color);
                }
            }
        }
        Ok(cage)
    }
}

/// Original unversioned GameState, including its obsolete derived hash.
#[derive(Encode, Decode)]
pub(crate) struct StateV0 {
    pub cage: CageV1,
    pub players: [PlayerV1; 2],
    pub remaining: [u8; 2],
    pub turn: PlayerV1,
    pub zobrist_hash: u64,
    pub last_move: Option<MoveV1>,
}
impl StateV0 {
    pub fn into_current(self) -> Result<GameState, &'static str> {
        state(
            self.cage,
            self.players,
            self.remaining,
            self.turn,
            self.last_move,
        )
    }
}

#[derive(Encode, Decode)]
pub(crate) struct SnapshotV1 {
    pub magic: [u8; 8],
    pub rules: u32,
    pub cage: CageV1,
    pub players: [PlayerV1; 2],
    pub remaining: [u8; 2],
    pub turn: PlayerV1,
    pub last_move: Option<MoveV1>,
}
impl SnapshotV1 {
    pub fn from_current(s: &GameState) -> Result<Self, &'static str> {
        s.validate()?;
        let colors = s
            .single_colors()
            .ok_or("Multi-color positions require the new snapshot format")?;
        let players = [
            PlayerV1 {
                id: 0,
                color: color_code(colors[0]),
            },
            PlayerV1 {
                id: 1,
                color: color_code(colors[1]),
            },
        ];
        Ok(Self {
            magic: *b"RCGPOS01",
            rules: SINGLE_COLOR_RULES,
            cage: s.cage.into(),
            players,
            remaining: colors.map(|c| s.remaining(c)),
            turn: players[s.player_to_move.id as usize],
            last_move: s.last_move.map(Into::into),
        })
    }
    pub fn into_current(self) -> Result<GameState, &'static str> {
        if self.magic != *b"RCGPOS01" || self.rules != SINGLE_COLOR_RULES {
            return Err("Unsupported saved-position rules");
        }
        state(
            self.cage,
            self.players,
            self.remaining,
            self.turn,
            self.last_move,
        )
    }
}

fn state(
    cage: CageV1,
    players: [PlayerV1; 2],
    remaining: [u8; 2],
    turn: PlayerV1,
    last_move: Option<MoveV1>,
) -> Result<GameState, &'static str> {
    if players[0].id != 0 || players[1].id != 1 || turn.id > 1 || turn != players[turn.id as usize]
    {
        return Err("Invalid legacy players");
    }
    let mut s = GameState::single_color(players.map(|p| color(p.color)), remaining)?;
    s.cage = cage.into_current()?;
    s.player_to_move = Player { id: turn.id };
    s.last_move = last_move.map(MoveV1::into_current).transpose()?;
    s.validate()?;
    s.rebuild_zobrist_hash();
    Ok(s)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(crate) struct KeyV1 {
    cage: CageV1,
    colors: [ColorV1; 2],
    remaining: [u8; 2],
    turn: u8,
    forbidden: Option<MoveV1>,
}
impl KeyV1 {
    pub fn from_current(key: PositionKey) -> Result<Self, &'static str> {
        let s = key.to_state();
        let colors = s
            .single_colors()
            .ok_or("Multi-color keys cannot be written as RCGEVAL1")?;
        Ok(Self {
            cage: s.cage.into(),
            colors: colors.map(color_code),
            remaining: colors.map(|c| s.remaining(c)),
            turn: s.player_to_move.id,
            forbidden: s.last_move.and_then(Move::inverse).map(Into::into),
        }
        .canonical())
    }
    pub fn into_current(self) -> Result<PositionKey, &'static str> {
        if self.turn > 1 || matches!(self.forbidden, Some(MoveV1::Drop { .. })) {
            return Err("Invalid legacy position key");
        }
        let players = [
            PlayerV1 {
                id: 0,
                color: self.colors[0],
            },
            PlayerV1 {
                id: 1,
                color: self.colors[1],
            },
        ];
        let forbidden = self.forbidden.map(MoveV1::into_current).transpose()?;
        let s = state(
            self.cage,
            players,
            self.remaining,
            players[self.turn as usize],
            forbidden.and_then(Move::inverse).map(Into::into),
        )?;
        if self.canonical() != self {
            return Err("Noncanonical legacy position key");
        }
        // Re-canonicalize in the live model rather than assuming identical key layouts.
        Ok(s.position_key())
    }
    /// Frozen v1 spatial canonicalization, independent of future live key ordering.
    fn canonical(self) -> Self {
        (0..8)
            .map(|symmetry| {
                let mut next = self;
                for x in 0..3 {
                    for y in 0..3 {
                        let (mut nx, mut ny) = (if symmetry >= 4 { 2 - x } else { x }, y);
                        for _ in 0..symmetry % 4 {
                            (nx, ny) = (ny, 2 - nx);
                        }
                        next.cage.0[nx][ny] = self.cage.0[x][y];
                    }
                }
                if symmetry >= 4
                    && let Some(MoveV1::RotateLayer { rotation, .. }) = &mut next.forbidden
                {
                    if *rotation < 2 {
                        *rotation ^= 1;
                    }
                }
                next
            })
            .max()
            .unwrap()
    }
}

#[derive(Clone, Copy, Encode, Decode)]
pub(crate) struct SpaceV1 {
    pub totals: [u8; 2],
    colors: [ColorV1; 2],
}
impl SpaceV1 {
    pub fn from_current(space: crate::search::packed::Space) -> Self {
        Self {
            totals: space.totals,
            colors: space.colors.map(color_code),
        }
    }
    pub fn into_current(self) -> Result<crate::search::packed::Space, &'static str> {
        if self.totals.iter().any(|&n| n > 24) || self.colors[0] == self.colors[1] {
            return Err("Invalid proof space");
        }
        Ok(crate::search::packed::Space {
            totals: self.totals,
            colors: [color(self.colors[0]), color(self.colors[1])],
        })
    }
}

#[derive(Clone, Copy, Encode, Decode)]
pub(crate) enum EvaluationV1 {
    Win { winner: u8, plies: u32 },
    Draw,
}
impl From<crate::search::Evaluation> for EvaluationV1 {
    fn from(value: crate::search::Evaluation) -> Self {
        match value {
            crate::search::Evaluation::Win { winner, plies } => Self::Win { winner, plies },
            crate::search::Evaluation::Draw => Self::Draw,
        }
    }
}
impl EvaluationV1 {
    pub fn into_current(self) -> Result<crate::search::Evaluation, &'static str> {
        match self {
            Self::Win { winner, plies } if winner < 2 => {
                Ok(crate::search::Evaluation::Win { winner, plies })
            }
            Self::Draw => Ok(crate::search::Evaluation::Draw),
            _ => Err("Invalid legacy winner"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_adapters_reject_invalid_codes_and_noncanonical_keys() {
        assert!(
            bincode::decode_from_slice::<ColorV1, _>(&[6], bincode::config::standard()).is_err()
        );
        for m in [
            MoveV1::RotateLayer {
                layer: 3,
                rotation: 0,
            },
            MoveV1::RotateLayer {
                layer: 0,
                rotation: 3,
            },
        ] {
            assert!(m.into_current().is_err());
        }
        let valid = KeyV1::from_current(GameState::new(3, 1).position_key()).unwrap();
        let mut invalid = valid;
        invalid.turn = 2;
        assert!(invalid.into_current().is_err());
        invalid = valid;
        invalid.forbidden = Some(MoveV1::Drop {
            color: ColorV1::Blue,
            column: (0, 0),
        });
        assert!(invalid.into_current().is_err());
        invalid = valid;
        invalid.forbidden = Some(MoveV1::RotateLayer {
            layer: 0,
            rotation: 2,
        });
        assert!(invalid.into_current().is_err());
        // With an empty board the reflected restriction breaks the orientation tie.
        invalid = valid;
        invalid.forbidden = Some(MoveV1::RotateLayer {
            layer: 0,
            rotation: 0,
        });
        assert_ne!(invalid, invalid.canonical());
        assert!(invalid.into_current().is_err());
        assert!(
            SpaceV1 {
                totals: [25, 0],
                colors: [ColorV1::Blue, ColorV1::Red]
            }
            .into_current()
            .is_err()
        );
        assert!(
            SpaceV1 {
                totals: [3, 1],
                colors: [ColorV1::Blue, ColorV1::Blue]
            }
            .into_current()
            .is_err()
        );
        assert!(
            EvaluationV1::Win {
                winner: 2,
                plies: 1
            }
            .into_current()
            .is_err()
        );
    }
}
