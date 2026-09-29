//! Repeatable native performance baseline for the single-color compatibility boundary.
//! cargo run --release --locked --example audit_compat
//! Timings are observational; exact answers, call counts and retained bounds must agree.
use rubik_cage::{
    core::game::GameState,
    search::{
        Evaluation,
        bounded::{Budget, Search},
        cache::Table,
    },
};
use std::time::{Duration, Instant};
fn report(label: &str, mut samples: Vec<Duration>) {
    samples.sort();
    println!(
        "{label}: min {:?}, median {:?}, max {:?}",
        samples[0],
        samples[samples.len() / 2],
        samples[samples.len() - 1]
    );
}
fn main() {
    let mut loads = Vec::new();
    let mut solves = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let table = Table::decode(include_bytes!("../assets/eval-v1.bin")).unwrap();
        loads.push(start.elapsed());
        assert_eq!(table.values.len(), 122_727);
        let root = GameState::new(12, 12);
        let mut search = Search::new(&root, Budget::default()).unwrap();
        let start = Instant::now();
        let result = search.exact(&root, 11).unwrap();
        solves.push(start.elapsed());
        assert_eq!(
            result,
            Some(Evaluation::Win {
                winner: 0,
                plies: 11
            })
        );
        println!(
            "Opening: {} calls, {} bound records",
            search.calls,
            search.proof.bounds.len()
        );
    }
    report("Bundled table decode", loads);
    report("Cache-free (12,12) opening", solves);
}
