//! Versioned saved positions, with a validated reader for the original GameState format.
use super::{
    cage::Cage,
    game::{GameState, Player},
    r#move::Move,
};
use bincode::{Decode, Encode};
const MAGIC: [u8; 8] = *b"RCGPOS01";

#[derive(Encode, Decode)]
struct Snapshot {
    magic: [u8; 8],
    rules: u32,
    cage: Cage,
    players: [Player; 2],
    remaining: [u8; 2],
    turn: Player,
    last_move: Option<Move>,
}

pub fn encode(state: &GameState) -> Result<Vec<u8>, String> {
    state.validate().map_err(str::to_owned)?;
    bincode::encode_to_vec(
        Snapshot {
            magic: MAGIC,
            rules: super::game::RULES_VERSION,
            cage: state.cage,
            players: state.players,
            remaining: state.remaining_cubies,
            turn: state.player_to_move,
            last_move: state.last_move,
        },
        bincode::config::standard(),
    )
    .map_err(|e| e.to_string())
}

pub fn decode(bytes: &[u8]) -> Result<GameState, String> {
    let config = bincode::config::standard().with_limit::<16384>();
    let (mut state, used) = if bytes.starts_with(&MAGIC) {
        let (snapshot, used): (Snapshot, usize) =
            bincode::decode_from_slice(bytes, config).map_err(|e| e.to_string())?;
        if snapshot.rules != super::game::RULES_VERSION {
            return Err("Unsupported saved-position rules".into());
        }
        (
            GameState {
                cage: snapshot.cage,
                players: snapshot.players,
                remaining_cubies: snapshot.remaining,
                player_to_move: snapshot.turn,
                last_move: snapshot.last_move,
                zobrist_hash: 0,
            },
            used,
        )
    } else {
        bincode::decode_from_slice::<GameState, _>(bytes, config).map_err(|e| e.to_string())?
    };
    if used != bytes.len() {
        return Err("Trailing data in saved position".into());
    }
    state.validate().map_err(str::to_owned)?;
    state.rebuild_zobrist_hash();
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_and_new_roundtrip_rebuild_derived_identity() {
        let mut state = GameState::new(3, 1);
        state.apply_move(Move::Flip).unwrap();
        let expected = state;
        state.zobrist_hash = 123;
        let legacy = bincode::encode_to_vec(state, bincode::config::standard()).unwrap();
        assert_eq!(decode(&legacy).unwrap(), expected);
        assert_eq!(decode(&encode(&state).unwrap()).unwrap(), expected);
        state.player_to_move.id = 42;
        let invalid = bincode::encode_to_vec(state, bincode::config::standard()).unwrap();
        assert!(decode(&invalid).is_err());
        let mut trailing = encode(&expected).unwrap();
        trailing.push(1);
        assert!(decode(&trailing).is_err());
    }
}
