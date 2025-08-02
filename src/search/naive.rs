use crate::game::GameState;
use std::collections::{HashMap, HashSet};

pub fn evaluate(game_state: &GameState) -> HashMap<GameState, isize> {
    let mut visited = HashSet::new();
    let mut evaluated = HashMap::new();
    minimax(game_state, &mut visited, &mut evaluated);
    evaluated
}

pub fn minimax(
    game_state: &GameState,
    visited: &mut HashSet<GameState>,
    evaluated: &mut HashMap<GameState, isize>,
) -> isize {
    if let Some(&score) = evaluated.get(game_state) {
        return score;
    }

    visited.insert(game_state.clone());

    if let Some(color) = game_state.cage.has_line() {
        let score = if color == game_state.players[0].color {
            1 // First player win
        } else {
            -1 // Second player win
        };
        visited.remove(game_state);
        evaluated.insert(game_state.clone(), score);
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
        new_game_state.apply_move(m).unwrap();
        if visited.contains(&new_game_state) {
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

    visited.remove(game_state);
    evaluated.insert(game_state.clone(), best_score);

    best_score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1_1_game_draw() {
        let game = GameState::new(1, 1);
        let evaluated = evaluate(&game);

        // 16 positions with single cubie, 72 with both, 1 empty, *2 for player_to_move
        assert_eq!(evaluated.len(), 178);
        assert_eq!(evaluated[&game], 0);
    }

    #[test]
    fn test_3_0_game_won_by_p1() {
        let game = GameState::new(3, 0);
        let evaluated = evaluate(&game);
        assert_eq!(evaluated[&game], 1);
    }

    #[test]
    fn test_1_4_game_won_by_p2() {
        let game = GameState::new(1, 4);
        let evaluated = evaluate(&game);
        assert_eq!(evaluated[&game], -1);
    }

    /// cargo test --release test_4_4_game -- --nocapture --ignored
    /// cargo flamegraph --unit-test -- test_4_4_game --ignored
    #[ignore]
    #[test]
    fn test_4_4_game() {
        // TODO: extract winning strategy
        let game = GameState::new(4, 4);
        let evaluated = evaluate(&game);
        println!("Game evaluation: {}", evaluated[&game]);
        println!("Number of evaluated states: {}", evaluated.len());
    }
}
