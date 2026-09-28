**Search soundness investigation — clean main `f202fe6`**

Historical report: the measurements and source-line references below describe the original implementation. `examples/audit_search.rs` now audits the corrected solver; its current output is intentionally different. See [implementation notes](search-implementation.md) for the new behavior and validation. The legacy asset is retained as a rejected-format fixture.

Minimax remains applicable to Rubik's Cage. The original single-pass DFS with cycle skipping and globally cached exact results is unsound, even in `Full` mode. Its pruning is independently unsound. Rebuilding the binary cache with the same solver cannot correct these problems.

Assumption: indefinitely avoiding a terminal win is a draw. The code's draw handling suggests this interpretation. The immediate-undo restriction does not eliminate longer cycles or moves that leave the board unchanged.

**Reproduce without cached data**

Run the independent small-graph diagnostic:

```sh
cargo run --release --locked --offline --example audit_search -- 3 0
cargo run --release --locked --offline --example audit_search -- 3 1
cargo run --release --locked --offline --example audit_search -- 0 3
```

Numeric arguments never load evaluation files. The reference enumerates the same graph as the current solver, then independently computes outcomes by synchronous fixed-point iteration. It checks that hash aliases have identical full states and that its final values satisfy the local minimax equations. It intentionally shares the existing transitions and hashes to isolate search errors, so these are results for the **currently implemented graph**, not certified solutions under corrected game rules. It limits enumeration to 100,000 states; use small inventories.

| Initial inventory | Graph states | Fixed-point root | `Full` root | `OptimalWL` root | `Pruned` root |
| --- | ---: | --- | --- | --- | --- |
| (3,0) | 60 | P1 win in 5 | P1 win in 5 | P1 win in 5 | P1 win in 5 |
| (3,1) | 538 | Draw | P1 win in 7 | P1 win in 9 | P1 win in 9 |
| (0,3) | 60 | P2 win in 6 | P2 win in 6 | P2 win in 6 | P2 win in 6 |

Even the apparently correct `(3,0)` root hides corrupted entries: `Full` caches the empty board with P2 to move as a draw, although every child is P1 win in 5. Re-evaluating that same state as a fresh root returns P1 win in 6. `Full` has 1 wrong winner entry; `OptimalWL` has 10. In `(3,1)`, the modes have 24, 55, and 35 wrong winner entries respectively. Those games cannot have simultaneous winners, so the terminal ambiguity described below does not affect these reproductions.

Distances are individual plies. `Pruned` deliberately promises bounds, so distance differences alone do not prove a bug in that mode. Wrong winners do.

**Why the original DFS fails**

In [naive.rs](../src/search/naive.rs), lines 148–150 discard edges leading to an ancestor. This changes the game being solved. Consider this alternating-turn graph:

```text
A (P1): move to B
B (P2): move to A, or to terminal P1 victory
```

P2 can avoid defeat forever by returning to A. Both states are draws. Searching A first discards B→A, then reports B as a P1 win in 1 and A as a P1 win in 2.

There is a second problem at line 220: evaluations computed with some legal successors excluded become globally reusable results. Consider A(P1) with moves to B(P2) or an immediate P1 win, and B's only move returning to A. Searching B first caches it as draw, then establishes A as a win. B is actually a P1 win in 2. Merely assigning a draw to every encountered backedge does not repair global memoization.

A cached result must be true for the state regardless of the DFS path. Unresolved cyclic dependencies need revisiting; the current implementation never revises them.

**Pruning fails independently of cycles**

Lines 168–175 set a cutoff whenever a P1-winning child appears, regardless of whose choice is being evaluated. Lines 67–72 then discard deeper continuations. This is not a valid alpha-beta bound.

An acyclic counterexample: A is P2's turn, with its first move losing immediately to P1 and its second move reaching B(P1), whose only move loses to P2. `Full` finds P2's win in 2. The pruning modes install a depth-1 cutoff after A's first move, discard B, and report P1's win in 1.

Also, pruned children return `None` but still make `no_children` false. If every child is pruned, the initial pessimistic score can become a fabricated loss in 1 and be stored as exact. The cutoff runs before terminal detection, and cache records do not distinguish exact results from bounds or unresolved work.

**The committed binary is independently inconsistent**

```sh
cargo run --release --locked --offline --example audit_search -- --cache
```

This separate mode loads only `assets/eval.bin` and replays actual UI moves with `apply_move`, preserving the previous-move restriction:

1. Blue drops at `(0,0)`.
2. Red drops at `(0,0)`.
3. Blue rotates the empty upper layer: `Clockwise` internally, labelled `Rotate Up CCW` by the UI.

The resulting position is Red's turn, hash `15851479399114258712`, cached as P1 win in 8. Red can legally Flip to hash `14384578360227930341`, cached as P1 win in 9. Therefore the parent must allow at least 10 plies under the cache's own values. All its other legal successors are cached as P1 win in 7.

This corresponds to the UI showing “Win in 8” on the upper-rotation row, then “Loss in 9” on Red's Flip row. It is a static cache contradiction; no background worker or new solver run is required.

The asset decodes fully into 375,294 entries (4,128,231 bytes); its root is P1 win in 9. It last changed in `90d95de` on 2025-08-24, before the current pruning change `1a396cc` on 2025-08-25. Full-search logic and hash generation did not materially change across those revisions. There is no evidence here of incompatible hash generation being the cause. The raw cache contains no solver/rules/hash/inventory version or exactness metadata.

[lib.rs](../src/lib.rs) embeds this map; [player.rs](../src/app/player.rs) requests fresh `OptimalWL` searches and blindly merges their results over existing entries. Thus path-dependent results from separate roots can overwrite each other. The per-panel worker flag also does not enforce the intended global single-worker guarantee. Move labels display the successor's distance, so an immediate win appears as “Win in 0”; that convention does not explain the increasing-distance contradiction.

**Game-state defects shared by both solvers**

- [game.rs](../src/core/game.rs), `apply_move_normalize` at line 157, never assigns `last_move`; `apply_move` does. Search keeps an obsolete undo restriction through descendants. From an initial state, normalized search even allows Flip immediately after Flip.
- `rebuild_zobrist_hash`, line 175, includes only board and player to move. Previous moves can change the legal move set, so those states must not share one exact cache entry. Remaining inventories also need inclusion or an explicitly scoped cache: they are inferable within one fixed initial inventory, but not across imported/different games. Canonicalization should account for the relevant move restriction as well as the board.
- [cage.rs](../src/core/cage.rs), `has_line`, returns the first winner found through a randomized `HashMap`. Legal rotations can create lines for both players. The reachable board `.........,.........,RBBR.BR.B` has both winners; independent probes selected different winners across processes and before/after normalization. A deterministic simultaneous-line rule must be specified and implemented. Board symmetry must preserve terminal outcomes.

These defects require correction before either solver can claim to evaluate the intended rules. The `(3,0)` and `(3,1)` diagnostics deliberately hold the faulty model constant and still demonstrate a separate algorithmic error.

**What can be retained, and the state of retrograde work**

Keep the minimax decision rule, but replace how cyclic dependencies are solved:

1. Establish terminal outcomes at distance 0.
2. A player can force a win when at least one successor is an established win for that player.
3. A player is forced to lose only when every successor is an established opponent win.
4. Propagate until no more states resolve. Remaining states are draws.
5. For decisive states, the winner minimizes distance and the loser maximizes it. Process by increasing distance so those values are exact.

This is the fixed-point/retrograde approach. It also provides a useful consistency certificate: a winner has a successor at distance `d-1`; a loser has only opponent-winning successors, all at most `d-1`, with at least one reaching that maximum. Decisive play therefore strictly decreases distance and cannot cycle.

A minimax-shaped implementation is also possible: search bounded horizons, key memoization by state and remaining horizon, and increase the horizon until the finite graph's winning regions stabilize (or a sufficient graph-size bound is reached). A short unresolved search is not proof of a draw. A history-aware repetition search can evaluate a root, but its intermediate history-dependent values cannot populate a universal state-only table. These approaches retain minimax semantics but do not rescue the present one-pass caching scheme.

The stashed `retrograde.rs` already implements graph enumeration, reverse edges, distance-ordered propagation, and unresolved-as-draw completion. Its propagation is conceptually sound for a correct complete two-player graph; duplicate edges are counted consistently. It inherits the game-state defects above and materializes the whole graph, so `(12,12)` feasibility and full correctness remain unverified. An isolated harness compared every WIP result with the independent reference: `(1,1)` (34 states), `(3,0)` (60), `(3,1)` (538), and `(0,3)` (60) all had zero score or distance mismatches. This validates the propagation on the current model, not the corrected physical rules. It is a suitable foundation after fixing the model. A later scalability option is solving strongly connected components or fixed-inventory layers, since drops decrease reserves and cannot participate in a cycle.

Before regenerating production assets: fix transitions, state identity and terminal rules; validate the graph solver on small games and every parent/child equation; then version the cache and keep exact entries distinct from search bounds. The README's claimed `(12,12)` solution should be treated as unverified.

**Workspace and validation**

Original work, including untracked retrograde code and binary files, is preserved in stash `6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689` (`stash@{0}` when created), named `Before naive search soundness investigation 2026-09-28`. It can be restored with `git stash apply 6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689`.

Production source and assets were left unchanged. Added this report and `examples/audit_search.rs`. The baseline `cargo test --locked --offline --lib -- --test-threads=1` yielded 19 passed, 1 failed, 1 ignored. The existing `test_1_1_game_draw` fails because its assertion expects P2 win in 0 instead of draw; that failure is an incorrect test expectation, separate from the reproduced search defects.
