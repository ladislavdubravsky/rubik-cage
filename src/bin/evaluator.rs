//! Generate, verify, and filter versioned exact evaluation tables.
use clap::{Parser, Subcommand};
use rubik_cage::{
    core::game::GameState,
    search::{
        EvaluationMap,
        cache::{Coverage, Table},
        retrograde::{self, Limits},
    },
};
use std::time::Instant;

#[derive(Parser)]
#[command(about = "Exact Rubik's Cage graph evaluator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Evaluate {
        p1_cubies: u8,
        p2_cubies: u8,
        outpath: String,
        #[arg(long, default_value_t = 100_000)]
        max_states: usize,
        #[arg(long, default_value_t = 1_500_000)]
        max_edges: usize,
    },
    Filter {
        infile: String,
        outfile: String,
        min_moves_to_wl: u32,
    },
    Verify {
        infile: String,
    },
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Evaluate {
            p1_cubies,
            p2_cubies,
            outpath,
            max_states,
            max_edges,
        } => {
            let initial = GameState::new(p1_cubies, p2_cubies);
            let started = Instant::now();
            let solution = retrograde::solve(
                &initial,
                &EvaluationMap::new(),
                Limits {
                    max_states,
                    max_edges,
                },
            )?;
            println!(
                "Root: {:?}; {:?}; solve time {:?}",
                solution.values[&initial.position_key()],
                solution.stats,
                started.elapsed()
            );
            let table = Table::complete(initial.position_key(), solution.values)?;
            table.save(outpath)?;
            println!(
                "Verified and saved {} exact evaluations",
                table.values.len()
            );
        }
        Command::Filter {
            infile,
            outfile,
            min_moves_to_wl,
        } => {
            let mut table = Table::load(infile)?;
            table.filter(min_moves_to_wl);
            table.save(outfile)?;
            println!(
                "Saved {} exact entries; omitted entries are unknown",
                table.values.len()
            );
        }
        Command::Verify { infile } => {
            let table = Table::load(infile)?;
            table.verify()?;
            println!("{} entries; {:?}", table.values.len(), table.coverage);
            if table.coverage == Coverage::Subset {
                println!(
                    "Format and retained entries validated; full minimax verification requires the complete source table"
                );
            }
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Evaluation failed: {error}");
        std::process::exit(1);
    }
}
