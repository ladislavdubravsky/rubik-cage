**Implementation status**

The correctness milestone and initial storage/browser integration are implemented. `naive.rs` remains available as historical source but is no longer part of the compiled library. The original user work remains preserved in stash `6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689`. Its retrograde file can be inspected with `git show 6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689^3:src/search/retrograde.rs`; applying the entire old stash over the replacement code would cause conflicts.

The implemented rule choice is that simultaneous lines are a draw, as provisionally specified in the plan. Infinite play is drawn, but a repeated state does not automatically end play. Changing the simultaneous-line rule requires changing the rules version and regenerating evaluation assets.

**Implemented contracts**

- UI and search use one validated, atomic move transition. It updates the previous-move restriction even when the board is unchanged. Both-player terminal lines are detected deterministically; terminal draws freeze the board as well as wins.
- Exact position keys contain board, turn, reserves, player colors and the prohibited inverse move. All horizontal symmetries transform board and restriction together. Full keys establish equality; Zobrist hashes remain only for compatibility with legacy positions.
- Retrograde explores all legal successors, deduplicates equivalent children, and propagates decisive outcomes by distance. Known exact boundaries retain their distances. Only the completed fixed point assigns unresolved states as draws. Limits return an error without publishing a partial table.
- The independent reference uses synchronous iteration. Tests compare every result on small games, vary move order, compare unreduced graphs with symmetry-reduced ones, reroot searches, and solve using cached boundaries. A separate verifier checks all terminal and minimax distance equations.
- Cache files carry rules/encoding/solver/distance versions and coverage. Complete tables are verified before writing or filtering. Legacy raw maps and incompatible/trailing data are rejected. Conflicting exact entries cannot overwrite one another.
- One app-level scheduler uses one shared worker. The worker retains exact cache boundaries. Responses carry request IDs; results are reusable by complete state key even after navigation. Labels include the proposed move. New saved positions omit derived hashes; legacy imports are validated and hashes rebuilt. Restart preserves initial inventories.

**Measured state spaces and current limits**

| Game | Result under corrected rules | States | Edges |
| --- | --- | ---: | ---: |
| `(3,1)` | P1 win in 9 plies | 2,668 | 18,753 |
| `(3,2)` | Draw | 14,704 | 111,271 |
| `(4,4)` | Unresolved at configured limit | 100,000 reached | 361,417 when interrupted |
| `(12,12)` | Unresolved at browser-sized limit | 20,000 reached | 63,236 when interrupted |

Native `(3,2)` evaluation, full verification and serialization used about 11 MiB peak RSS and 1.0 CPU second in this environment. The interrupted `(4,4)` and `(12,12)` probes used about 20 MiB and 7 MiB respectively. These bounded probes are not estimates of complete game sizes. Timing was collected while build jobs were also running, so wall times are not benchmark-quality latency claims.

The included `assets/eval-v1.bin` is a 105 KiB verified complete `(3,1)` table. It deliberately cannot satisfy an empty `(12,12)` lookup because reserves belong to the key. The old asset is not a fallback.

Native defaults are 100,000 states / 1,500,000 edges; browser defaults are 20,000 / 300,000. These cap graph work, not an exact allocator byte count or elapsed-time deadline. Runtime exact caches are trimmed at 100,000 entries while retaining the newest solution. Large games remain playable, with unknown evaluations after the budget is reached.

The next performance work is compact node/predecessor storage and solving inventory layers or strongly connected components. Full `(12,12)` precomputation, production-sized regenerated assets, and an optional bounded minimax accelerator are not implemented or claimed solved.

**Validation**

Native regression suite: all 32 tests passed, including all library, binary, and example targets. Both release WASM entry points build successfully. The headless Chrome smoke test passed: the original replay, the full optimal 9-ply game, bounded unknown evaluations, one reused worker, fresh `(3,2)` solving, legacy import, undo/restart, and simultaneous draws.

The browser was served from WASM bindings generated with `wasm-bindgen-cli 0.2.100` (matching Cargo.lock); Trunk itself was not installed in this environment. The smoke script also supports a normal Trunk-served build.

Strict Clippy (`-D warnings`) reports pre-existing style lints in rendering, helpers and coordinate loops. The solver/model correctness checks and browser checks pass; unrelated style cleanup is not part of this change.
