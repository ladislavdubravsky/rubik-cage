//! Generate, verify, and filter versioned exact evaluation tables.
use clap::{Parser, Subcommand};
use rubik_cage::{
    core::game::GameState,
    search::{
        EvaluationMap,
        bounded::{Budget, Proof, Search},
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
    /// Prove wins with finite horizons and draws with closed safety strategies.
    Precompute {
        p1_cubies: u8,
        p2_cubies: u8,
        outpath: String,
        #[arg(long)]
        proof: String,
        #[arg(long, default_value_t = 4)]
        opening_plies: u8,
        #[arg(long, default_value_t = 18, value_parser = clap::value_parser!(u8).range(..255))]
        max_horizon: u8,
        #[arg(long, default_value_t = 1_000_000)]
        max_positions: usize,
        #[arg(long, default_value_t = 100_000_000)]
        max_calls: u64,
        #[arg(long, default_value_t = 2_000_000)]
        calls_per_position: u64,
    },
    /// Verify horizon and safety claims against core, then certify retained table entries.
    VerifyProof {
        proof: String,
        #[arg(long)]
        table: Option<String>,
    },
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
        Command::Precompute {
            p1_cubies,
            p2_cubies,
            outpath,
            proof,
            opening_plies,
            max_horizon,
            max_positions,
            max_calls,
            calls_per_position,
        } => {
            if outpath == proof {
                return Err("Table and proof paths must differ".into());
            }
            let root = GameState::new(p1_cubies, p2_cubies);
            let mut search = Search::new(
                &root,
                Budget {
                    max_positions,
                    max_calls,
                },
            )?;
            let start = Instant::now();
            let pos = search.proof.space.encode(&root);
            let mut positions = vec![(pos, 0u8)];
            let mut seen = std::collections::HashSet::from([pos]);
            let mut index = 0;
            while index < positions.len() {
                let (position, depth) = positions[index];
                if depth < opening_plies {
                    let children = search.proof.space.children(position);
                    for &child in &children.items[..children.len] {
                        if !seen.contains(&child) {
                            if positions.len() >= max_positions {
                                return Err("Opening enumeration exceeds --max-positions; use fewer opening plies".into());
                            }
                            seen.insert(child);
                            positions.push((child, depth + 1));
                        }
                    }
                }
                index += 1;
            }
            let mut solved = 0;
            let mut limited = 0;
            for (index, (position, _)) in positions.iter().enumerate() {
                let state = search.proof.space.decode(*position);
                search.budget.max_calls =
                    max_calls.min(search.calls.saturating_add(calls_per_position));
                match search.exact(&state, max_horizon) {
                    Ok(Some(_)) => solved += 1,
                    Ok(None) => (),
                    Err(_) => limited += 1,
                }
                if index % 100 == 0 {
                    println!(
                        "Opening {index}/{}; solved={solved}; bounds={}; calls={}; elapsed={:?}",
                        positions.len(),
                        search.proof.bounds.len(),
                        search.calls,
                        start.elapsed()
                    );
                }
                if search.calls >= max_calls {
                    break;
                }
            }
            search.budget.max_calls = max_calls;
            if let Err(error) = search.refine_wins() {
                println!("Refinement incomplete: {error}");
            }
            println!(
                "Opening solved {solved}/{}; budget-limited queries={limited}; bounds={}; calls={}; search time={:?}",
                positions.len(),
                search.proof.bounds.len(),
                search.calls,
                start.elapsed()
            );
            println!("Closed {} safety claims", search.proof.close_safety());
            println!("Verifying certificate against ordinary core transitions...");
            search.proof.verify()?;
            let values = search.proof.exact_values();
            println!(
                "Root: {:?}; {} exact entries; missing entries remain unknown",
                values.get(&root.position_key()),
                values.len()
            );
            let opening_covered = positions
                .iter()
                .filter(|(p, _)| {
                    let state = search.proof.space.decode(*p);
                    rubik_cage::search::Evaluation::terminal(&state).is_some()
                        || values.contains_key(&state.position_key())
                })
                .count();
            println!(
                "Certified opening coverage: {opening_covered}/{}",
                positions.len()
            );
            search.proof.save(&proof)?;
            Table {
                roots: vec![root.position_key()],
                coverage: Coverage::Subset,
                values,
            }
            .save(outpath)?;
            println!(
                "Verified proof and table saved; total time {:?}",
                start.elapsed()
            );
        }
        Command::VerifyProof { proof, table } => {
            let proof = Proof::load(&proof)?;
            println!(
                "Verified {} horizon bounds and {} safety claims against core",
                proof.bounds.len(),
                proof.safe.len()
            );
            if let Some(path) = table {
                let table = Table::load(path)?;
                let certified = proof.exact_values();
                for (key, value) in &table.values {
                    if certified.get(key) != Some(value) {
                        return Err(format!("Table entry lacks an exact proof: {key:?}").into());
                    }
                }
                println!(
                    "Certified all {} retained table entries",
                    table.values.len()
                );
            }
        }
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
                    "Format and retained entries validated; minimax verification requires the complete source table or verify-proof with its certificate"
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
