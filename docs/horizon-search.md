# Compact minimax and the corrected (12,12) precomputation

Under rules version 1 (simultaneous lines and indefinite play draw, immediate inverse prohibited, unchanged-board rotations legal), P1 can force a win from `(12,12)` in **11 plies**, and cannot force it within 10. A corner drop preserves that result. The opening's edge drops remain unknown at horizon 18; they are not classified as draws.

`bounded.rs` asks a different question from the old DFS: can a specified player force victory within a finite number of plies? At that player's turn the answer is the OR of child answers; at the opponent's turn it is their AND. Each recursive call decreases the horizon. Cycles are revisited with less time, never removed from the graph.

Each memo entry holds two explicit monotone bounds: no victory within `not_within - 1`, and a forced victory within `within`. These are independent of the path used to reach the state. Only matching adjacent bounds give an exact distance. A failed horizon, exhausted work budget, or unresolved cycle cannot establish a draw. Complete retrograde analysis remains available for that purpose.

A search namespace fixes total inventories and player colors. Within it, a canonical 52-bit key contains both 24-bit boards, the turn, and the forbidden inverse move. Reserves are inferred from total inventory minus board population. Board and restriction are transformed together. Exported evaluations still use the complete public keys, including inventories and colors; the compact integers never become global cache keys. Move generation uses bit operations, precomputed byte symmetries, and stack-based child ordering.

Every bound has an independently checked certificate. The verifier reconstructs ordinary `GameState`s, calls their legal move generator and transition implementation, and checks the AND/OR witnesses using strictly decreasing requested horizons. It does not use packed move generation or its terminal classifier. Necessary material/turn-count lower bounds provide base cases. Refinement closes the gaps on positive bounds, yielding exact distances throughout the proved winning strategies, including every defense at a losing position.

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
| All canonical positions through four opening plies | 804 of 849 proved within horizon 18 |
| After distance refinement | 427,586 bound records, 2,819,554 calls |
| Search | about 4.7 seconds |
| Search, independent verification, and artifact writes | about 11.5 seconds |
| Published table | 122,245 exact decisive entries, about 5.5 MiB |
| Independently checkable proof | about 4.5 MiB; not embedded in WASM |

These counts describe proof coverage, not enumeration of the entire reachable `(12,12)` graph. The table covers the opening winning strategy and many alternatives. Missing entries remain unknown. The separate complete `(3,1)` table is also retained by the browser.

Native regressions compare packed transitions and terminal results with core on every state in several small complete games, compare horizon answers with exact graph values, check the `(12,12)` winning strategy and all its defenses, reject a corrupted proof, and verify that interruption never produces a draw. The browser regression plays an optimal 11-ply `(12,12)` game and checks losing replies against the displayed distances.

The shared browser worker retains compact bounds only within a matching inventory/color namespace. Each request allows 1,000,000 horizon calls, up to 250,000 retained bounds, and horizon 18. Remaining unknowns can use graph analysis capped at 20,000 states / 300,000 edges. Exact partial results are published even when other moves remain unknown; an incomplete root is not automatically resubmitted in a loop.
