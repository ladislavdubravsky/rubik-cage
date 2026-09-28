**Plan: exact minimax on the Rubik's Cage game graph**

Use the preserved `retrograde.rs` as the production implementation of graph minimax. Keep the minimax choice rule; replace the one-pass DFS, ancestor-edge deletion, and depth cutoff. Establish correctness on small games before scaling or regenerating browser assets.

This document records the original plan; see [implementation status](search-implementation.md) for completed work and remaining performance work. The original work remains in stash `6bafb16d9a3b6ad487a45e1b04c7f6755b3e8689`. During implementation, recover the retrograde source selectively; do not restore its old binaries or UI switches wholesale.

**Rules and result contract**

| Topic | Planned contract |
| --- | --- |
| Endless play | Draw. Do not introduce an automatic repetition-ending rule. |
| Simultaneous lines | Explicit decision requested from the user; provisional planning assumption is draw. Resolve before finalizing terminal behavior or generating assets. |
| Immediate undo | Keep the existing rule: prohibit the inverse of the preceding move, including on otherwise unchanged boards. |
| Empty-layer rotations / other unchanged-board moves | Preserve their current legality; they still change turn and the next move restriction. |
| Distance | Count individual plies. Terminal decisive states have distance 0. A proposed move's label includes that move, so an immediate win is shown as win in 1. |
| Solved value | Exact `Win { winner, plies }` or `Draw`. Unknown/incomplete is a separate status, never a draw or a fabricated distance. |
| Scope | Two players, one color per player. No multiplayer or multiple-color expansion in this work. |

The rule decision about simultaneous lines affects terminal classification; the rest of the architecture supports any deterministic choice. The actual rule, including its tie behavior, becomes part of cache compatibility.

1. **Repair the game model and add focused regressions.**

   Main files: `src/core/game.rs`, `src/core/cage.rs`, `src/core/move.rs`, and relevant tests.

   Make ordinary play and search call one authoritative move implementation. Validate legality before mutating the state, update the previous-move restriction, apply the board operation and gravity, and advance the turn consistently. Canonicalization becomes a separate operation after that transition. A failed move leaves the entire state unchanged.

   Replace first-match winner selection with deterministic detection of all winning players, then apply the agreed simultaneous-line rule. Represent terminal draws as well as wins, and give terminal states no legal continuations. Preserve physical orientation for rendering and move input.

   Add regressions for Flip-after-Flip, prohibited inverse rotations, unchanged-board moves, inventory depletion, invalid moves, simultaneous lines, and equality between ordinary transitions and search transitions after canonicalization. Correct the existing `(1,1)` test's mistaken expected value. Treat old solver outputs for other inventories as evidence of bugs, not as authoritative expected results for corrected rules.

   Completion criterion: terminal outcomes and move legality agree between UI and search, across repeated runs and supported symmetries.

2. **Replace hash-only identity with a complete canonical position key.**

   Main files: `src/core/game.rs`, a small position/key module if useful, and search map types.

   Introduce an equality-checkable `PositionKey` containing board, side to move, remaining inventories, and the relevant prohibited reply. Include player/color assignments unless fixed by the key's documented namespace. After a drop there is no inverse prohibition, so the drop's original column is not part of search identity. Keep full last-move information separately if presentation or history needs it.

   A Zobrist hash may accelerate lookup, but cannot alone establish identity. Start with an explicit key; pack it only after measuring memory, preserving exact equality and a stable encoding. Do not trust a derived hash loaded from a saved position.

   Canonicalize board and prohibited reply jointly over the supported horizontal symmetries. On symmetric boards, use the restriction to break ties as well. Reflection reverses rotation direction. Keep this transformation explicit rather than relying on the existing board-only normalizer's boolean result.

   Verify normalization idempotence, equal keys for equivalent positions, different keys for positions with different legal futures, and equivalent sets of canonical legal successors under symmetry. Also compare small games with symmetry reduction disabled. Because Flip uses a fixed horizontal axis, test equivalence of canonical successors rather than assuming every raw transition commutes with the same spatial transform.

   Completion criterion: keys identify complete rule states and symmetry reduction preserves the game graph and terminal outcomes.

3. **Adapt retrograde into the exact graph solver and independently verify it.**

   Main files: selectively recovered `src/search/retrograde.rs`, `src/search/mod.rs`, shared evaluation types, and test support derived from `examples/audit_search.rs`.

   Separate graph construction from outcome propagation sufficiently to test propagation on tiny artificial graphs. Build nodes using the new keys and every legal successor. Deduplicate equivalent successor keys and predecessor links consistently. Seed terminal decisive states at distance 0 and terminal draws as draws. Process decisive states in increasing distance: one winning successor proves a win for the mover; every successor must prove defeat before declaring a forced loss. Winner chooses the shortest win, loser the longest unavoidable loss. Mark remaining unresolved states as drawn only after the complete reachable graph reaches its fixed point.

   Define explicit limits for graph size/memory/work and an incomplete result when they are reached. The first version can return no new table on interruption; it must not publish provisional values as exact. Expose solved values independently of the old `SearchMode` enum. Remove the original pruning modes from the production evaluation path when integrating the new API.

   Keep the synchronous fixed-point reference separate from production propagation. Test cycles with an escape, forced exits, pure drawing cycles, transpositions reached in different orders, opponent alternatives, competing win distances, longest forced losses, duplicate moves, and terminal draws. Compare every value on small corrected Cage graphs and reorder moves to check order independence.

   Independently check the local equations for every solved state. For decisive distance d, the winner has a best child at d-1; a losing mover has only opponent-winning children, their maximum distance is d-1. A nonterminal draw has no winning child for its mover and at least one draw-preserving child. Distances along a forced winning strategy strictly decrease. Run fresh-root evaluations of sampled states and compare them with entries from a larger solve.

   Completion criterion: production and reference results agree throughout small graphs; all certificates and fresh-root comparisons pass. The corrected `(12,12)` result remains unknown until actually solved.

4. **Measure feasibility and add exact cache boundaries.**

   Main files: solver internals, `src/bin/evaluator.rs`, and a reproducible benchmark/audit command.

   Measure graph nodes, edges, solve time, and peak memory for increasing inventories and representative later-game positions. Evaluate native precomputation and browser-query workloads separately. Do not assume the full-graph implementation matches the old pruned solver's speed.

   Let an already certified exact cache entry stop graph expansion at that state. A decisive boundary retains its real remaining distance in the priority queue; a cached win in 12 is not a terminal win in 0. Known draw boundaries remain draws. Every newly expanded node still requires all legal outgoing edges. Compare solving with and without exact cached boundaries on small games.

   If measurements require it, use compact node indices and predecessor storage first. Next consider solving reserve layers or strongly connected components: drops strictly reduce reserves, so cycles are confined to layers without drops. Continue comparing optimized results with the baseline solver.

   Bounded recursive minimax is a possible later query accelerator, not a required second production solver. If introduced, its memoization includes remaining horizon, bounds have explicit types, and an unresolved horizon never becomes an exact draw. Avoid reintroducing the old cutoff or publishing intermediate history-dependent values.

   Completion criterion: recorded native and browser resource requirements determine supported inventories and query limits. Expensive requests remain explicitly incomplete if those limits are exceeded; `(12,12)` support is not promised without evidence.

5. **Introduce a versioned evaluation format and regenerate verified data.**

   Main files: shared search/cache module, `src/bin/evaluator.rs`, `src/lib.rs`, and `assets/`.

   Replace the raw hash-to-evaluation file with a header and exact-key entries. Identify format, rules, key/canonicalization version, distance convention, and compatible solver semantics; record generator provenance and coverage/initial inventories. Distinguish a full table from a filtered subset so missing entries always mean unknown, never draw.

   Reject the old unversioned evaluation binaries rather than converting their values. Preserve them only as investigation fixtures outside the production load path. Add an evaluator verification command that checks full-table certificates before filtering. Preserve compatibility metadata during filtering, validate decoding and complete input consumption, and publish the new file only after validation succeeds. Start with a small verified asset; generate `(12,12)` only if phase 4 establishes feasibility.

   For the same rules and exact key, two exact entries must agree. A conflict is a diagnostic error; keep it visible instead of silently overwriting one answer with another.

   Completion criterion: incompatible assets cannot enter a running evaluation map, round trips preserve values, and regenerated data passes the same checks as fresh solves.

6. **Integrate the browser, migrate saved positions, and verify the original symptom.**

   Main files: `src/app/agent.rs`, `src/app/player.rs`, `src/app/utils.rs`, `src/app/game_control.rs`, `src/bin/worker.rs`, and `src/lib.rs`.

   Move evaluation scheduling to one app-level owner. Panels consume results; they do not independently launch competing searches. Send canonical position/rules identity and a request identifier to the worker. A result for an older visible position must not finish or repaint a newer request. Same-rules exact entries may still be reused safely by their complete keys. Reuse compatible known values as solver boundaries without copying an ever-growing table for every individual move request.

   Render unknown, computing, incomplete, draw, and decisive results distinctly. Show exact distances only for solved values, including the selected move in its label. Use the same typed values for sorting and display. Finish games on terminal draws as well as wins. A missing or incompatible cache leaves the app playable with unknown evaluations until a solve completes.

   Version exported/local-storage position data independently of evaluation caches. Provide a reader for valid legacy positions where possible, recomputing derived identity and validating player/color mapping, inventories and the relevant last move. Report unsupported or invalid input clearly rather than treating old bytes as the new format. Keep undo, restart, and imported inventories consistent.

   Test the two-drops/upper-rotation/Flip regression, an immediate win, simultaneous-line termination, drawing cycles, undo/restart/import while work is pending, and cache-hit versus fresh-solve agreement. For a displayed best win in n, verify the resulting losing player's best delaying reply is loss in n-1 and every other reply loses at least as quickly. Use the agreed move-label convention consistently.

   Completion criterion: native tests and the WASM build pass; browser flows agree with the solver; the original increasing-distance symptom is absent in verified positions. Update README solution claims using newly verified results only.

**Execution order and scope**

Implement phases 1–3 first as the correctness milestone. Phases 4–6 establish performance, trusted storage, and the usable web integration. Introduce cache rejection before switching the app onto corrected state semantics, so interim development builds do not mix new states with legacy evaluations. Each phase should be independently reviewable, while preserving the original stash until its useful work has been recovered.

No production implementation or expensive precomputation is part of this planning turn.
