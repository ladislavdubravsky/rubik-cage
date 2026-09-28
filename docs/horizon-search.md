# Compact minimax and the corrected (12,12) precomputation

Under rules version 1 (simultaneous lines and indefinite play draw, immediate inverse prohibited, unchanged-board rotations legal), P1 can force a win from `(12,12)` in **11 plies**, and cannot force it within 10. A corner drop preserves that result. Every edge drop is a proved draw. Flip and every rotation lose in 12 including the selected move; all 15 opening moves now have exact labels.

`bounded.rs` asks a different question from the old DFS: can a specified player force victory within a finite number of plies? At that player's turn the answer is the OR of child answers; at the opponent's turn it is their AND. Each recursive call decreases the horizon. Cycles are revisited with less time, never removed from the graph.

Each memo entry holds two explicit monotone bounds: no victory within `not_within - 1`, and a forced victory within `within`. These are independent of the path used to reach the state. Only matching adjacent bounds give an exact distance. A failed horizon, exhausted work budget, or unresolved cycle cannot establish a draw. Draws require complete retrograde analysis or the separate safety certificates described below.

A search namespace fixes total inventories and player colors. Within it, a canonical 52-bit key contains both 24-bit boards, the turn, and the forbidden inverse move. Reserves are inferred from total inventory minus board population. Board and restriction are transformed together. Exported evaluations still use the complete public keys, including inventories and colors; the compact integers never become global cache keys. Move generation uses bit operations, precomputed byte symmetries, and stack-based child ordering.

Every bound has an independently checked certificate. The verifier reconstructs ordinary `GameState`s, calls their legal move generator and transition implementation, and checks the AND/OR witnesses using strictly decreasing requested horizons. It does not use packed move generation or its terminal classifier. Necessary material/turn-count lower bounds provide base cases. Refinement closes the gaps on positive bounds, yielding exact distances throughout the proved winning strategies, including every defense at a losing position.

## Draw certificates

After horizon search and distance refinement, `close_safety` finds closed strategies that prevent a specified target player from ever winning. At the target's turn **every** legal child must remain safe; at the defender's turn **at least one** child must remain safe. Terminal target wins are unsafe. Terminal draws, opponent wins, insufficient target material, and independently proved opponent victories can serve as safe boundaries.

The extractor starts with candidate positions from the explored bounds and repeatedly removes candidates whose obligations are not met. Missing successors are unsafe. A finite negative horizon is never itself a safety boundary. Cycles may remain only when all of these closure conditions hold. A state is exported as a draw only when both players have independently certified strategies to prevent the other's victory.

The verifier reconstructs ordinary core states and checks every closure obligation. Positive horizon proofs are well-founded and do not depend on safety claims; safety may use these proved victories as boundaries. Negative horizon claims can in turn use certified safety. This prevents circular reasoning between the two proof kinds.

Proof format `RCGPRF02` adds the safety sets; the loader still accepts and verifies `RCGPRF01` horizon-only proofs. The exact evaluation-table format and rules version are unchanged.

## Reproducing the shipped artifacts

```sh
cargo run --release --bin evaluator -- precompute 12 12 assets/eval-v1.bin \
  --proof assets/eval-v1.proof.bin --opening-plies 4 --max-horizon 18
cargo run --release --bin evaluator -- verify-proof assets/eval-v1.proof.bin \
  --table assets/eval-v1.bin
```

Default resource caps are 1,000,000 bound records, 100,000,000 recursive calls overall, and 2,000,000 calls per opening position. Use `--max-positions`, `--max-calls`, and `--calls-per-position` to change them. Unfinished claims are omitted; independently verified exact results may still be saved as a subset when the budget runs out. `evaluate` remains the complete graph precomputation command. `verify` checks file compatibility and structural validity; use `verify-proof --table` to certify the minimax values in a horizon-generated subset.

Measured on this development machine (release build):

| Work | Result |
| --- | --- |
| Opening result alone | 4,874 bound records, 21,072 calls, about 35 ms |
| Canonical positions through four opening plies | 804 decisive + 41 draws = 845 of 849 proved |
| After distance refinement | 427,586 bound records, 2,819,554 calls |
| Closed safety claims | 233,206 player-specific claims |
| Horizon search | about 6 seconds |
| Search, safety closure, independent verification, and writes | about 17 seconds |
| Published table | 122,245 decisive + 482 draws = 122,727 exact entries, about 5.5 MiB |
| Independently checkable proof | about 6.5 MiB; not embedded in WASM |

These counts describe proof coverage, not enumeration of the entire reachable `(12,12)` graph. The table covers every initial move, the opening winning strategy and many alternatives. Four positions within those four opening plies still lack proofs. Missing entries remain unknown. The separate complete `(3,1)` table is also retained by the browser.

Native regressions compare packed transitions and terminal results with core on every state in several small complete games, compare horizon answers with exact graph values, check the `(12,12)` winning strategy and all its defenses, reject corrupted bounds and forged safety sets, compare safety closure with all drawn states of complete small graphs, and verify that interruption never produces a draw. The browser regression checks all 15 initial move labels and an edge-drop draw, then plays an optimal 11-ply `(12,12)` game and checks losing replies against the displayed distances.

The shared browser worker retains compact bounds only within a matching inventory/color namespace. Each request allows 1,000,000 horizon calls, up to 250,000 retained bounds, and horizon 18. After horizon queries, it also closes safety strategies to certify available draws for custom inventories. Remaining unknowns can use graph analysis capped at 20,000 states / 300,000 edges. Exact partial results are published even when other moves remain unknown; an incomplete root is not automatically resubmitted in a loop.
