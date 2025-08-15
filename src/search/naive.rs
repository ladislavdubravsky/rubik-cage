use crate::core::game::GameState;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

// TODO: track W/L/D in how many moves
pub fn evaluate(game_state: &GameState) -> HashMap<u64, isize> {
    let mut visited = HashSet::new();
    let mut evaluated = HashMap::new();
    let mut game_state = game_state.clone();
    game_state.normalize();

    minimax(&game_state, &mut visited, &mut evaluated);
    evaluated
}

pub fn minimax(
    game_state: &GameState,
    visited: &mut HashSet<u64>,
    evaluated: &mut HashMap<u64, isize>,
) -> isize {
    if let Some(&score) = evaluated.get(&game_state.zobrist_hash) {
        return score;
    }

    visited.insert(game_state.zobrist_hash);

    if let Some(color) = game_state.cage.has_line() {
        let score = if color == game_state.players[0].color {
            1 // First player win
        } else {
            -1 // Second player win
        };
        visited.remove(&game_state.zobrist_hash);
        evaluated.insert(game_state.zobrist_hash, score);
        return score;
    }

    let mut best_score = if game_state.player_to_move.id == 0 {
        -1
    } else {
        1
    };
    let mut no_children = true;
    let moves = game_state.legal_moves();
    for m in moves {
        let mut new_game_state = game_state.clone();
        new_game_state.apply_move_normalize(m).unwrap();
        if visited.contains(&new_game_state.zobrist_hash) {
            continue;
        }

        no_children = false;
        let score = minimax(&new_game_state, visited, evaluated);

        if game_state.player_to_move.id == 0 {
            best_score = best_score.max(score);
            if best_score == 1 {
                break;
            }
        } else {
            best_score = best_score.min(score);
            if best_score == -1 {
                break;
            }
        }
    }

    // We found no positions we haven't seen and no win along the way, so it must be draw
    if no_children {
        best_score = 0;
    }

    visited.remove(&game_state.zobrist_hash);
    evaluated.insert(game_state.zobrist_hash, best_score);

    best_score
}

pub fn save_eval(map: &HashMap<u64, isize>, path: &str) -> Result<(), Box<dyn Error>> {
    let config = bincode::config::standard();
    let encoded: Vec<u8> = bincode::encode_to_vec(map, config)?;
    let mut file = File::create(path)?;
    file.write_all(&encoded)?;

    Ok(())
}

pub fn load_eval(path: &str) -> Result<HashMap<u64, isize>, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let config = bincode::config::standard();
    let (decoded_map, _len): (HashMap<u64, isize>, usize) =
        bincode::decode_from_slice(&buffer, config)?;

    Ok(decoded_map)
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn test_1_1_game_draw() {
        let game = GameState::new(1, 1);
        let evaluated = evaluate(&game);

        // 16 positions with single cubie, 72 with both, 1 empty, *2 for player_to_move
        assert_eq!(evaluated.len(), 178);
        assert_eq!(evaluated[&game.zobrist_hash], 0);
    }

    #[test]
    fn test_3_0_game_won_by_p1() {
        let game = GameState::new(3, 0);
        let evaluated = evaluate(&game);
        assert_eq!(evaluated[&game.zobrist_hash], 1);
    }

    #[test]
    fn test_1_4_game_won_by_p2() {
        let game = GameState::new(1, 4);
        let evaluated = evaluate(&game);
        assert_eq!(evaluated[&game.zobrist_hash], -1);
    }

    /// cargo test --release test_4_4_game -- --nocapture --ignored
    /// cargo flamegraph --unit-test -- test_4_4_game --ignored
    #[ignore]
    #[test]
    fn test_4_4_game() {
        let game = GameState::new(4, 4);
        let evaluated = evaluate(&game);
        println!("Game evaluation: {}", evaluated[&game.zobrist_hash]);
        println!("Number of evaluated states: {}", evaluated.len());

        save_eval(&evaluated, "eval.bin").unwrap();
        let loaded = load_eval("eval.bin").unwrap();
        assert_eq!(loaded, evaluated);
    }

    /// cargo test --release test_12_12_game -- --nocapture --ignored
    #[ignore]
    #[test]
    fn test_12_12_game() {
        let game = GameState::new(12, 12);
        let stack_size = 32 * 1024 * 1024;
        let evaluated = thread::Builder::new()
            .stack_size(stack_size)
            .spawn(move || evaluate(&game))
            .unwrap()
            .join()
            .unwrap();
        println!("Game evaluation: {}", evaluated[&game.zobrist_hash]);
        println!("Number of evaluated states: {}", evaluated.len());

        save_eval(&evaluated, "eval.bin").unwrap();
    }
}
