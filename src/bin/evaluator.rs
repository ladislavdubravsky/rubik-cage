//! This binary enables precomputing evaluations for a number of "most difficult" game states.
//! The evaluations are stored in a file and included in the webapp binary.
//! Evaluation of all states of the (12, 12) game takes somewhat longish and storing all of them
//! is a bit too sizey, so this is our compromise: load several MB of precomputed evaluations
//! and compute smaller targeted remainders on the fly in a web worker.

use clap::{Parser, Subcommand};
use rubik_cage::{
    core::game::GameState,
    search::naive::{Evaluation, SearchMode, evaluate, load_eval, save_eval},
};
use std::{collections::HashMap, thread};

// TODO: analyze positions which take the longest to win etc.
// TODO: table of results for (m, n) games
// TODO: allow arbitrary (m, n) games in UI

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Evaluate the game where players start with `p1_cubies` and `p2_cubies` respectively
    /// and save results to `outpath`.
    /// Example: `evaluator evaluate 12 12 eval/eval_12_12.bin`
    Evaluate {
        /// Number of cubies for player 1
        p1_cubies: u8,
        /// Number of cubies for player 2
        p2_cubies: u8,
        /// Output file path
        outpath: String,
    },
    /// Filter an existing evaluation file by minimum moves to win/loss.
    /// Example: `evaluator filter eval/eval_12_12.bin assets/eval.bin 3`
    Filter {
        /// Input file path
        infile: String,
        /// Output file path
        outfile: String,
        /// Minimum moves to win/loss
        min_moves_to_wl: isize,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Evaluate {
            p1_cubies,
            p2_cubies,
            outpath,
        } => {
            eval(&outpath, p1_cubies, p2_cubies);
        }
        Commands::Filter {
            infile,
            outfile,
            min_moves_to_wl,
        } => {
            filter(&infile, &outfile, min_moves_to_wl);
        }
    }
}

/// Evaluate a specific (m, n) game and store the results in a binary file.
fn eval(file: &str, p1_cubies: u8, p2_cubies: u8) {
    let game = GameState::new(p1_cubies, p2_cubies);
    let stack_size = 32 * 1024 * 1024;
    let evaluated = thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || evaluate(&game, SearchMode::Full))
        .unwrap()
        .join()
        .unwrap();
    let eval = evaluated[&game.zobrist_hash];
    let eval_str = match eval.score {
        1 => format!("Player 1 win in {} moves", eval.moves_to_wl),
        -1 => format!("Player 2 win in {}", eval.moves_to_wl),
        0 => "Draw".to_string(),
        ev => format!("Unexpected evaluation: {:?}", ev),
    };

    println!("Game evaluation: {}", eval_str);
    println!("Number of evaluated states: {}", evaluated.len());

    save_eval(&evaluated, file).unwrap();
}

/// Filter computed evaluations from `file` to retain positions that take long to win (lose), i.e.,
/// hopefully, the evaluations that take the longest to compute. We try to leave ourselves positions
/// that are easy to evaluate for on-the-fly evaluation in the webapp.
fn filter(file: &str, out_file: &str, min_moves_to_wl: isize) {
    let eval = load_eval(file).unwrap();
    let filtered: HashMap<u64, Evaluation> = eval
        .into_iter()
        .filter(|(_k, v)| v.moves_to_wl >= min_moves_to_wl)
        .collect();
    println!("Filtered number of states: {}", filtered.len());
    save_eval(&filtered, out_file).unwrap();
}
