//! Versioned saved positions with frozen, validated single-color compatibility readers.
use super::game::GameState;
use crate::compat::{CageV1, MoveV1, SnapshotV1, StateV0};
use crate::core::game::Player;
use bincode::{Decode, Encode};

#[derive(Encode, Decode)]
struct SnapshotV2 {
    magic: [u8; 8],
    rules: u32,
    cage: CageV1,
    owners: [Option<u8>; 6],
    remaining: [u8; 6],
    turn: u8,
    last_move: Option<MoveV1>,
}

pub fn encode(state: &GameState) -> Result<Vec<u8>, String> {
    state.validate().map_err(str::to_owned)?;
    if state.single_colors().is_some() {
        let snapshot = SnapshotV1::from_current(state).map_err(str::to_owned)?;
        return bincode::encode_to_vec(snapshot, bincode::config::standard())
            .map_err(|e| e.to_string());
    }
    let snapshot = SnapshotV2 {
        magic: *b"RCGPOS02",
        rules: super::game::RULES_VERSION,
        cage: state.cage.into(),
        owners: state.color_owners,
        remaining: state.remaining_cubies,
        turn: state.player_to_move.id,
        last_move: state.last_move.map(Into::into),
    };
    bincode::encode_to_vec(snapshot, bincode::config::standard()).map_err(|e| e.to_string())
}

pub fn decode(bytes: &[u8]) -> Result<GameState, String> {
    let config = bincode::config::standard().with_limit::<16384>();
    let (state, used) = if bytes.starts_with(b"RCGPOS02") {
        let (snapshot, used): (SnapshotV2, usize) =
            bincode::decode_from_slice(bytes, config).map_err(|e| e.to_string())?;
        if snapshot.rules != super::game::RULES_VERSION {
            return Err("Unsupported saved-position rules".into());
        }
        let mut state =
            GameState::with_colors(snapshot.owners, snapshot.remaining).map_err(str::to_owned)?;
        state.cage = snapshot.cage.into_current().map_err(str::to_owned)?;
        state.player_to_move = Player { id: snapshot.turn };
        state.last_move = snapshot
            .last_move
            .map(MoveV1::into_current)
            .transpose()
            .map_err(str::to_owned)?;
        state.validate().map_err(str::to_owned)?;
        state.rebuild_zobrist_hash();
        (Ok(state), used)
    } else if bytes.starts_with(b"RCGPOS01") {
        let (snapshot, used): (SnapshotV1, usize) =
            bincode::decode_from_slice(bytes, config).map_err(|e| e.to_string())?;
        (snapshot.into_current(), used)
    } else {
        let (raw, used): (StateV0, usize) =
            bincode::decode_from_slice(bytes, config).map_err(|e| e.to_string())?;
        (raw.into_current(), used)
    };
    if used != bytes.len() {
        return Err("Trailing data in saved position".into());
    }
    state.map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::r#move::Move;
    #[test]
    fn multicolor_snapshot_validation_rejects_invalid_ownership_stock_and_turn() {
        let bytes = encode(&GameState::multicolor()).unwrap();
        for fault in 0..4 {
            let (mut wire, _): (SnapshotV2, usize) =
                bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
            match fault {
                0 => wire.owners[0] = Some(2),
                1 => wire.owners[0] = None,
                2 => wire.remaining[0] = 255,
                _ => wire.turn = 2,
            }
            let invalid = bincode::encode_to_vec(wire, bincode::config::standard()).unwrap();
            assert!(decode(&invalid).is_err());
        }
    }

    #[test]
    fn exported_positions_rebuild_derived_identity() {
        let mut state = GameState::new(3, 1);
        state.apply_move(Move::Flip).unwrap();
        let expected = state;
        state.zobrist_hash = 123;
        assert_eq!(decode(&encode(&state).unwrap()).unwrap(), expected);
        let mut trailing = encode(&expected).unwrap();
        trailing.push(1);
        assert!(decode(&trailing).is_err());
    }
}
