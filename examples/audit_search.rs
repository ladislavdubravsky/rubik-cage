//! Cache-free solve and full certificate check under the corrected rules.
//! cargo run --release --example audit_search -- 3 1
use rubik_cage::{
    core::game::GameState,
    search::{
        EvaluationMap,
        retrograde::{Limits, solve, verify},
    },
};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let p1 = args.get(1).map(|v| v.parse()).transpose()?.unwrap_or(3);
    let p2 = args.get(2).map(|v| v.parse()).transpose()?.unwrap_or(1);
    let initial = GameState::new(p1, p2);
    let start = Instant::now();
    let solution = solve(&initial, &EvaluationMap::new(), Limits::default())?;
    println!(
        "Root {:?}; {:?}; time {:?}",
        solution.values[&initial.position_key()],
        solution.stats,
        start.elapsed()
    );
    verify(&solution.values)?;
    println!("Every terminal, outcome, distance, and successor equation verified");
    Ok(())
}
