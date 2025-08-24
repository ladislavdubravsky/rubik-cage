use crate::core::game::GameState;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct Evaluation {
    /// 1 = P1 win, -1 = P2 win, 0 = draw
    pub score: isize,
    /// If the position is drawn, -1. If the position is won (lost), upper bound on number of
    /// moves to force a win (lower bound on number of moves to lose). If the search was done
    /// without pruning the bounds are exact under optimal play.
    pub moves_to_wl: isize,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchMode {
    /// Search all reachable positions. For instance, this will go past one move wins (as if the
    /// player missed the opportunity). It will however not continue playing an already won game.
    Full,
    /// If a win is seen, this keeps searching to make sure there is not a faster win. Unlike the
    /// previous option, it will not go past one move wins, as no shorter ones can be found there.
    OptimalWL,
    /// This will search until we know the position evaluation with certainty, but doesn't try
    /// to find optimal moves to W/L.
    Pruned,
}

impl std::fmt::Display for Evaluation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "score: {}, moves_to_wl: {}",
            self.score, self.moves_to_wl
        )
    }
}

pub fn evaluate(game_state: &GameState, mode: SearchMode) -> HashMap<u64, Evaluation> {
    // visited tracks states seen in a *particular* game, to avoid searching cycles
    let mut visited = HashSet::new();
    let mut evaluated = HashMap::new();
    let mut game_state = game_state.clone();
    game_state.normalize();

    minimax(&game_state, &mut visited, &mut evaluated, &mode);
    evaluated
}

pub fn minimax(
    game_state: &GameState,
    visited: &mut HashSet<u64>,
    evaluated: &mut HashMap<u64, Evaluation>,
    mode: &SearchMode,
) -> Evaluation {
    // Check if we've either seen this position or win is on board
    if let Some(&eval) = evaluated.get(&game_state.zobrist_hash) {
        return eval;
    }

    visited.insert(game_state.zobrist_hash);

    if let Some((winner, _)) = game_state.won() {
        let score = if winner.id == game_state.players[0].id {
            1 // First player win
        } else {
            -1 // Second player win
        };
        visited.remove(&game_state.zobrist_hash);
        let eval = Evaluation {
            score,
            moves_to_wl: 0,
        };
        evaluated.insert(game_state.zobrist_hash, eval);
        return eval;
    }

    // If we didn't resolve the position yet, evaluate all children
    // Each player can lose or better
    let mut best_score = match game_state.player_to_move.id {
        0 => -1,
        1 => 1,
        _ => unreachable!(),
    };

    // If we're P1 and there are wins, track the fastest. If loss is the best we can do, track
    // the slowest. If draw mark as -1. For P2 vice versa.
    let mut moves_to_wl_p1win_max = 0;
    let mut moves_to_wl_p1win_min = isize::MAX;
    let mut moves_to_wl_p1loss_max = 0;
    let mut moves_to_wl_p1loss_min = isize::MAX;

    let mut no_children = true;
    let moves = game_state.legal_moves();

    // if we see a 1 move win, prune everything else unless in exhaustive search mode
    if mode != &SearchMode::Full {
        for m in &moves {
            let mut new_game_state = game_state.clone();
            new_game_state.apply_move_normalize(*m).unwrap();
            if let Some((winner, _)) = new_game_state.won() {
                if game_state.player_to_move.id == winner.id {
                    let eval = Evaluation {
                        score: match game_state.player_to_move.id {
                            0 => 1,
                            1 => -1,
                            _ => unreachable!(),
                        },
                        moves_to_wl: 1,
                    };
                    visited.remove(&game_state.zobrist_hash);
                    evaluated.insert(game_state.zobrist_hash, eval);
                    return eval;
                }
            }
        }
    }

    for m in moves {
        let mut new_game_state = game_state.clone();
        new_game_state.apply_move_normalize(m).unwrap();
        if visited.contains(&new_game_state.zobrist_hash) {
            continue;
        }

        no_children = false;
        let eval = minimax(&new_game_state, visited, evaluated, mode);

        if eval.score == 1 {
            moves_to_wl_p1win_max = moves_to_wl_p1win_max.max(eval.moves_to_wl);
            moves_to_wl_p1win_min = moves_to_wl_p1win_min.min(eval.moves_to_wl);
        }
        if eval.score == -1 {
            moves_to_wl_p1loss_max = moves_to_wl_p1loss_max.max(eval.moves_to_wl);
            moves_to_wl_p1loss_min = moves_to_wl_p1loss_min.min(eval.moves_to_wl);
        }

        if game_state.player_to_move.id == 0 {
            best_score = best_score.max(eval.score);
            if best_score == 1 && mode == &SearchMode::Pruned {
                break;
            }
        } else {
            best_score = best_score.min(eval.score);
            if best_score == -1 && mode == &SearchMode::Pruned {
                break;
            }
        }
    }

    // We found no positions we haven't seen and no win along the way, so it must be draw
    if no_children {
        best_score = 0;
    }

    let moves_to_wl: isize = match (best_score, game_state.player_to_move.id) {
        // P1 wins and is to move: track fastest win
        (1, 0) => moves_to_wl_p1win_min + 1,
        // P1 wins, P2 to move: track slowest loss
        (1, 1) => moves_to_wl_p1win_max + 1,
        // P2 wins, P1 to move: track fastest win
        (-1, 0) => moves_to_wl_p1loss_max + 1,
        // P2 wins and is to move: track fastest win
        (-1, 1) => moves_to_wl_p1loss_min + 1,
        // Draw
        _ => -1, 
    };

    let eval = Evaluation {
        score: best_score,
        moves_to_wl,
    };

    visited.remove(&game_state.zobrist_hash);
    evaluated.insert(game_state.zobrist_hash, eval);

    eval
}

pub fn save_eval(map: &HashMap<u64, Evaluation>, path: &str) -> Result<(), Box<dyn Error>> {
    let config = bincode::config::standard();
    let encoded: Vec<u8> = bincode::encode_to_vec(map, config)?;
    let mut file = File::create(path)?;
    file.write_all(&encoded)?;

    Ok(())
}

pub fn load_eval(path: &str) -> Result<HashMap<u64, Evaluation>, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let config = bincode::config::standard();
    let (decoded_map, _len): (HashMap<u64, Evaluation>, usize) =
        bincode::decode_from_slice(&buffer, config)?;

    Ok(decoded_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1_1_game_draw() {
        let game = GameState::new(1, 1);
        let evaluated = evaluate(&game, SearchMode::Full);
        assert_eq!(
            evaluated[&game.zobrist_hash],
            Evaluation {
                score: 0,
                moves_to_wl: 0
            }
        );
    }

    #[test]
    fn test_3_0_game_won_by_p1() {
        let game = GameState::new(3, 0);
        let evaluated = evaluate(&game, SearchMode::Full);
        assert_eq!(
            evaluated[&game.zobrist_hash],
            Evaluation {
                score: 1,
                moves_to_wl: 5
            }
        );
    }

    #[test]
    fn test_1_4_game_won_by_p2() {
        let game = GameState::new(1, 4);
        let evaluated = evaluate(&game, SearchMode::Pruned);
        assert_eq!(evaluated[&game.zobrist_hash].score, -1);
    }

    /// cargo test --release test_4_4_game -- --nocapture --ignored
    /// cargo flamegraph --unit-test -- test_4_4_game --ignored
    #[ignore]
    #[test]
    fn test_4_4_game() {
        let game = GameState::new(4, 4);
        let evaluated = evaluate(&game, SearchMode::Pruned);
        println!("Game evaluation: {}", evaluated[&game.zobrist_hash]);
        println!("Number of evaluated states: {}", evaluated.len());

        save_eval(&evaluated, "eval.bin").unwrap();
        let loaded = load_eval("eval.bin").unwrap();
        assert_eq!(loaded, evaluated);

        std::fs::remove_file("eval.bin").unwrap();
    }
}
