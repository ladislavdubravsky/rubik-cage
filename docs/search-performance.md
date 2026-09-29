# Step 4: measured general-search optimization

Measured locally on 2026-09-29, Linux x86_64, Intel Core i7-7500U (4 logical CPUs), Rust 1.98.1, Chrome 154.0.8037.57, release builds. These are single-machine measurements, not portable performance guarantees.

## Result and enabled changes

The production general solver now uses 76-bit color-aware positions in a `u128`: 24 three-bit cells, the player to move, and the forbidden inverse. Its namespace fixes ownership and all six total inventories. Reserves follow from board counts; no colors are merged into player occupancy masks. Bounds use bit 76 for the proof target, giving a 77-bit claim key. Public/UI cache keys and saved files remain complete, unchanged `PositionKey`s.

Spatial transforms use row rotations and an 8 KiB lookup table. Color relabeling sorts sparse occupancy masks only within equal-owner, equal-total classes. This computes the same canonical result as first-occurrence labeling without enumerating color permutations. Unequal inventories cannot be exchanged. Equivalent requested moves share one query, but results are published under every original full key.

Move ordering favors monochromatic threats and treats another friendly color as a blocker. It is only an ordering heuristic: finite-horizon proof rules, exact-distance checks and draw rules are unchanged. Proof verification still uses ordinary core moves, outcomes and material bounds, independently of compact transitions and terminal detection. The tiny incremental graph solver remains core-based.

The existing two-color backend, its budgets, assets and saved formats are unchanged. Compact keys never enter legacy tables or proof formats.

## End-to-end measurements

Each trial uses the deployed initial allowance: up to 32 batches, 4,000 microsteps per batch, a cooperative 40 ms batch target, 25,000 bounds, 16 MiB estimated memory and horizon 6. It stops early on completion, depth exhaustion or capacity. Times below are medians of three trials; memory is the median ending estimate in KiB; maximum batch time is the largest observed optimized batch across the three trials. Coverage counts original legal moves, including exact terminal moves.

**These times measure a bounded search allowance, not a full solution.** Opening, midgame and late-game moves remain Unknown at this allowance. No cutoff is presented as a draw.

| Platform / fixture | Reference → optimized ms | Speedup | Estimated KiB | Max batch ms | Solved moves |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native opening | 469 → 38 | 12.45× | 1714 → 88 | 11 | 0/31 |
| Native midgame | 698 → 360 | 1.94× | 3095 → 1268 | 23 | 0/31 |
| Native late | 60 → 22 | 2.78× | 346 → 134 | 7 | 0/7 |
| Native tactic | 558 → 271 | 2.06× | 2523 → 815 | 12 | 1/31 |
| Native small | 29 → 24 | 1.20× | 1724 → 1699 | 25 | 23/23 |
| WASM opening | 540 → 88 | 6.14× | 1403 → 88 | 28 | 0/31 |
| WASM midgame | 905 → 728 | 1.24× | 2535 → 1259 | 40 | 0/31 |
| WASM late | 120 → 46 | 2.61× | 283 → 134 | 15 | 0/7 |
| WASM tactic | 664 → 566 | 1.17× | 2068 → 808 | 30 | 1/31 |
| WASM small | 36 → 31 | 1.16× | 1328 → 1309 | 31 | 23/23 |


Coverage was unchanged relative to the baseline on these fixtures. The small fixture has nonterminal exact wins for all 23 moves. The tactical fixture has one immediately winning move; it is not evidence of deeper solved-move coverage. Continuing search can explore further, but this audit does not claim to solve the full multi-color opening.

The fixtures are reproducible in `examples/audit_general.rs`: the requested 18-piece opening, an eight-drop midgame, a legal nonterminal position reached by continuing drops until no further nonterminal drop is available, a two-Green immediate threat, and a small White-three/Blue-one game with no opposing stock. All colors remain explicitly assigned, so the small case uses the general backend.

## Ablations and diagnostics

Raw three-trial data: [native CSV](benchmarks/general-native.csv), [WASM CSV](benchmarks/general-wasm.csv). The four backends are:

- `reference`: preserved step-3 core-based solver, compiled only with `search-audit`.
- `compact`: compact transitions and keys, spatial symmetry, terminal-first ordering.
- `relabel`: additionally enables safe color relabeling and shared equivalent move queries.
- `ordered`: additionally enables monochromatic threat ordering; this is the deployed configuration.

CSV columns include microsteps, expanded positions, horizon-query calls, cache hits, bound records, estimated bytes, median/p95/max batch latency and solved-move coverage. Expansion rate is `expansions * 1000 / ms`; query cache hit rate is `cache_hits / calls`. Calls include terminal/material shortcuts, so this rate is not the hit rate conditioned on actually probing the table. A microstep can expand one node or traverse one return edge; it is not synonymous with a node.


| Platform / fixture | Expansions/s reference → optimized | Query cache hit % reference → optimized |
| --- | ---: | ---: |
| native opening | 14,072 → 37,064 | 9.1 → 4.0 |
| native midgame | 16,757 → 34,278 | 8.3 → 5.7 |
| native late | 37,381 → 108,240 | 12.2 → 10.9 |
| native tactic | 14,220 → 36,090 | 3.6 → 4.0 |
| native small | 5,817 → 4,872 | 14.5 → 17.9 |
| wasm opening | 12,233 → 15,875 | 9.1 → 4.0 |
| wasm midgame | 12,922 → 16,938 | 8.3 → 5.7 |
| wasm late | 18,650 → 50,630 | 12.2 → 10.9 |
| wasm tactic | 11,943 → 17,249 | 3.6 → 4.0 |
| wasm small | 4,667 → 3,774 | 14.5 → 17.9 |

Compact-only remains faster than full relabeling on some midgames. Relabeling earns a large reduction near the symmetric opening, while the enabled combination remains faster and smaller than the baseline on every measured fixture. The first implementation used per-cell color mapping; measurements exposed its midgame overhead, so it was replaced with sparse-mask sorting before the recorded final audit. Further adaptive policies need separate evidence and equivalence tests.

Memory is conservative application accounting, not measured browser heap usage. It excludes the UI cache, the separate classic solver and runtime overhead. Browser timers have millisecond resolution; first-run compilation, OS scheduling and parallel build activity can affect results. Trial order is fixed and the CSV retains each trial rather than hiding variability.

## Reproduce

Native:

```sh
cargo run --release --features search-audit --example audit_general > /tmp/general-native.csv
```

WASM (use the wasm-bindgen version matching Cargo.lock; Trunk already caches this version):

```sh
cargo build --release --locked --target wasm32-unknown-unknown --features search-audit --example audit_general
mkdir -p /tmp/cage-search-audit
~/.cache/trunk/wasm-bindgen-0.2.100/wasm-bindgen --target web --out-dir /tmp/cage-search-audit --out-name audit_general target/wasm32-unknown-unknown/release/examples/audit_general.wasm
cp scripts/audit_general.html /tmp/cage-search-audit/index.html
python3 -m http.server 8777 --bind 127.0.0.1 --directory /tmp/cage-search-audit
```

In another terminal:

```sh
node scripts/browser_audit.mjs http://127.0.0.1:8777/ > /tmp/general-wasm.csv
```

The audit compares every overlapping exact result across variants and fails on conflicts. The benchmark and frozen reference are excluded from normal app builds. No benchmark code is exposed in the product UI.

## Correctness and remaining scope

Regression tests compare compact successors, outcomes and canonicalization with core across complete small graphs and deterministic full-game walks. They cover all six colors, 55-move configurations, unequal inventories, exhausted reserves, inverse restrictions, both-owner wins, all 36 preset color permutations, and all four combinations of relabeling/ordering. Exact distances are checked against complete graph solutions; interrupted and corrupted proofs retain the independent core verifier.

Step 4 does not add heuristic evaluations, generalized disk proof/table formats, larger precomputed bundles, or a full solution of the multi-color opening. Those remain optional follow-up studies. Larger cyclic draw certificates also remain future work; existing material and complete-small-graph draw proofs are preserved.
