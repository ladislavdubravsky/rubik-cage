**Implementation status**

The correctness milestone, storage/browser integration, and compact finite-horizon optimization are implemented. `naive.rs` remains available as historical source but is no longer part of the compiled library. The original user work remains preserved in stash `6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689`. Its retrograde file can be inspected with `git show 6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689^3:src/search/retrograde.rs`; applying the entire old stash over the replacement code would cause conflicts.

The implemented rule choice is that simultaneous lines are a draw, as provisionally specified in the plan. Infinite play is drawn, but a repeated state does not automatically end play. Changing the simultaneous-line rule requires changing the rules version and regenerating evaluation assets.

**Implemented contracts**

- UI and reference verification use one validated, atomic move transition. Packed search transitions are checked exhaustively against it on small graphs. It updates the previous-move restriction even when the board is unchanged. Both-player terminal lines are detected deterministically; terminal draws freeze the board as well as wins.
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
| `(12,12)` graph probe | Unresolved at browser-sized limit | 20,000 reached | 63,236 when interrupted |

Native `(3,2)` evaluation, full verification and serialization used about 11 MiB peak RSS and 1.0 CPU second in this environment. The interrupted `(4,4)` and `(12,12)` probes used about 20 MiB and 7 MiB respectively. These bounded probes are not estimates of complete game sizes. Timing was collected while build jobs were also running, so wall times are not benchmark-quality latency claims.

The larger-game optimizer now proves `(12,12)` as P1 win in 11 plies using finite-horizon minimax and a compact 52-bit position representation scoped to inventories/colors. It does not need to enumerate the entire graph to prove a forced win. Adjacent failed/successful horizons establish exact distances; only complete graph solving can establish nonterminal draws. See [the detailed algorithm and reproducible measurements](horizon-search.md).

`assets/eval-v1.bin` contains 122,245 exact `(12,12)` entries, certified by `assets/eval-v1.proof.bin` against ordinary core moves and terminal rules. Search took about 4.7 seconds; search plus verification and writes took about 11.5 seconds. The table is explicitly a subset, with 804 of 849 positions through four opening plies solved. The `(3,1)` complete table remains in `assets/eval-3-1.bin`. The legacy binary is never a fallback.

Native graph defaults remain 100,000 states / 1,500,000 edges. The browser combines horizon search (1,000,000 calls per request, 250,000 retained bounds, horizon 18) with graph fallback (20,000 states / 300,000 edges). These bound work and retained records, not exact allocator bytes or elapsed time. Exact caches are trimmed at 500,000 entries, preserving the shipped opening data and the current UI move values. Incomplete responses can publish proved values while leaving other moves unknown; they do not trigger an automatic retry loop.

Full enumeration of the `(12,12)` graph and layer/component decomposition remain optional future optimizations. The opening's outcome is now proved, independently of those larger exhaustive computations.

**Validation**

Native regression suite: all 37 tests passed, including all library, binary, and example targets. Both release WASM entry points build successfully. The headless Chrome smoke test passed: the original replay, the optimal 11-ply `(12,12)` game with every losing reply checked, the full optimal 9-ply `(3,1)` game, unknown alternatives, one reused worker, fresh `(3,2)` solving, legacy import, undo/restart, and simultaneous draws.

The browser was served from WASM bindings generated with `wasm-bindgen-cli 0.2.100` (matching Cargo.lock); Trunk itself was not installed in this environment. The smoke script also supports a normal Trunk-served build.

Strict Clippy (`-D warnings`) reports pre-existing style lints in rendering, helpers and coordinate loops. The solver/model correctness checks and browser checks pass; unrelated style cleanup is not part of this change.
