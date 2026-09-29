//! Checked boundary between the live game model and the specialized two-color solver.
//! Future multi-color states must be rejected here, never collapsed to owner boards.
use crate::core::{cage::Cage, cubie::Cubie, game::GameState, r#move::Move};

#[derive(Clone, Copy)]
pub(super) struct SingleColorState {
    pub cage: Cage,
    pub colors: [Cubie; 2],
    pub remaining: [u8; 2],
    pub turn: u8,
    pub last_move: Option<Move>,
}
impl TryFrom<&GameState> for SingleColorState {
    type Error = &'static str;
    fn try_from(state: &GameState) -> Result<Self, Self::Error> {
        state.validate()?;
        let colors = state
            .single_colors()
            .ok_or("Multi-color evaluation is not available yet")?;
        Ok(Self {
            cage: state.cage,
            colors,
            remaining: colors.map(|c| state.remaining(c)),
            turn: state.player_to_move.id,
            last_move: state.last_move,
        })
    }
}
impl SingleColorState {
    pub fn totals(self) -> [u8; 2] {
        let mut totals = self.remaining;
        for color in self.cage.grid.iter().flatten().flatten().flatten() {
            let owner = self
                .colors
                .iter()
                .position(|c| c == color)
                .expect("Validated single-color board");
            totals[owner] += 1;
        }
        totals
    }
    pub fn into_current(self) -> GameState {
        let mut state = GameState::single_color(self.colors, self.remaining)
            .expect("Validated single-color stocks");
        state.cage = self.cage;
        state.player_to_move = state.players[self.turn as usize];
        state.last_move = self.last_move;
        state.rebuild_zobrist_hash();
        state
    }
}
