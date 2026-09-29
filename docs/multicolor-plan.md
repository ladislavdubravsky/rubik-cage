# Multi-color games: feasibility and implementation plan

Status: steps 1–4 implemented. Multi-color games support play, per-color settings and persistence, plus resumable exact evaluation with core verification, incremental tiny graph solves, proof verification and pause/resume. Difficult evaluations remain Unknown. The specialized two-color solver and legacy artifacts are preserved. See [compatibility regression gates](single-color-compatibility.md) and [general search behavior and limits](general-search.md). Compact color-aware search, safe color relabeling and threat ordering are measured on native and WASM; see [step-4 results](search-performance.md). Larger offline studies remain optional step 5.

## Scope and recommendation

Feasible. Keep two alternating players, allow each to own several distinct colors, and require three cubies of one exact color for a winning line. Mixed-color lines do not win, even if every cubie belongs to the same player.

The initial presets should be the existing one-color-per-player game, including its current 0–12 inventory controls, and the requested three-colors-per-player game with three cubies of each color (18 total). Advanced configuration can assign the six existing colors to either player and specify stocks per color. Ownership must be exclusive and fixed during a game. This proposal does not add more players or change gravity, legal rotations, flipping, immediate-undo restrictions, or the convention that lines for both players produce a draw. Two winning colors belonging to one player still produce that player's win.

Ship playable games independently of deep evaluation. Preserve the specialized two-color search backend, add a correct general path, then optimize multi-color evaluation based on measurements. Full solution of the requested opening is an investigation, not a release prerequisite or promised result.

## What the code already supports

| Area | Current implementation | Implication |
| --- | --- | --- |
| Colors and board | `Cubie` has six colors; `Cage` stores actual colors | Reuse board representation, gravity, flips and rotations |
| Lines | `Cage::lines()` finds monochromatic lines using 28 geometric lines | Reuse line detection; map resulting colors to their owners |
| Moves | `Move::Drop` already includes a color | No new drop move variant required |
| Game state | `Player.color`, two reserve counts, and a full `Player` as turn | Separate player identity from color ownership and stock |
| Position identity | Two colors, two reserve counts, turn, board and inverse restriction | Generalize complete keys and player-swap transforms |
| Retrograde search | Expands through core moves and outcomes | Its two-player propagation remains applicable |
| Bounded search | Directly uses a two-board packed representation | Needs a separate generalized representation/backend |
| Browser evaluation | Shared worker, partial results, continuation, explicit Unknown | Reuse the lifecycle, strengthen scheduling and resource bounds |
| Rendering | CSS and `slot_to_css` support only Blue and Red | All six colors need visible, accessible rendering |
| Persistence | Versioned snapshots/tables/proofs plus raw legacy state import | Explicit compatibility readers are essential |

`src/search/naive.rs` is historical and is not compiled. Work should target `retrograde.rs`, `bounded.rs`, and `packed.rs`.

## Core model

Use player IDs for identity and turn. A practical fixed-size model is a six-entry color-owner map (`Option<PlayerId>`), six reserve counts, the existing cage, turn, and previous move. An unassigned color must have neither reserve nor board pieces. Fixed-size arrays preserve cheap state copies. Keep `GameState::new(m, n)` as the existing Blue/Red constructor; add a validated configuration constructor.

Prefer one authoritative ownership mapping over storing redundant color sets in several objects. Player display metadata should not define legal ownership. Give the core helpers for colors owned by a player, reserve counts, total inventory by color, and initial-state reconstruction. Settings currently use search's `Space` to recover initial stocks; remove this gameplay-to-search dependency.

Required changes:

- Generate a drop for every available owned color in each non-full column. Check ownership and stock atomically when applying a move, then decrement that color's stock.
- Determine outcomes by collecting owners of monochromatic lines. Preserve simultaneous-opponent-line draws and terminal move rejection.
- Return winning color as well as player and line for rendering; optionally expose all winning lines.
- Validate ownership, per-color inventories, board colors, gravity, turn and last-drop ownership. A previous drop may use any color owned by the previous player. Preserve the existing accepted single-color import inventory range rather than silently applying the UI's narrower limit to old files.
- Restart by returning board pieces to reserves of their exact colors, then resetting cage, turn and restriction. Undo naturally restores the complete state.
- Keep selected drop color in UI state, outside game identity and search keys.

Keys must distinguish board colors, ownership, reserves, turn and immediate-inverse restriction. Starting totals can be reconstructed from board counts plus reserves under these no-capture rules. If packed keys omit totals or ownership, the search namespace must include them. Never merge states merely because their player occupancy masks match.

## UI and interaction

Keep the existing game as the default and preserve its direct click-to-drop behavior. Add presets to Game settings, with per-color stocks for the multi-color preset and, subsequently, advanced configuration.

Each player panel shows their colors and remaining counts. During their turn, select an available color and click a column. Auto-select the sole available color; keep or repair selection after a turn change, stock exhaustion, undo, import, restart or preset change. Reset stale hover previews when their move ceases to apply. Exhausted colors remain visible but disabled. No reserve does not end the game: rotations and flips remain possible.

Complete all six CSS colors and `slot_to_css`, including reserve icons, board cells and previews. Use color names or symbols in addition to swatches, with keyboard-operable selectors and clear selection/disabled states. Player identity should remain clear independently of the selected piece color. Announce wins as, for example, “Player 1 wins with Green.”

Drop rows need both color and column. In the requested opening there are 24 drops plus seven manipulation moves, so grouping/filtering drop rows by color will help. Rotations and flips appear once. Every legal move remains clickable regardless of whether its evaluation is known; keep the current convention that displayed distances include the selected move.

## Search: what changes and what remains valid

The game remains deterministic, perfect-information and two-player. Existing minimax AND/OR reasoning, distance propagation, finite-horizon proofs and closed safety strategies remain applicable. Adding colors does not require a multiplayer equilibrium algorithm.

### Retrograde

The current retrograde solver delegates transitions to core and uses full position keys. After those are generalized, it is the natural correctness reference for tiny multi-color games. Its complete-graph requirement makes it unsuitable as the default exhaustive strategy for the 18-piece preset. An interrupted expansion must still publish no speculative draws. The existing worker discards failed graph expansions; avoid repeatedly rebuilding the same oversized graph, or retain a budgeted frontier in a later improvement.

### Bounded proofs and optimized representation

`packed.rs` stores two 24-bit player boards, a turn and a three-bit restriction in 52 bits. It reconstructs reserves from two totals. It also has fixed 15-child arrays. `bounded.rs` stores the proof target in bit 52. These assumptions cannot represent distinct colors owned by one player.

Preserve this backend behind a checked adapter that only accepts games with at most one assigned color per player in its supported configuration. Use the same public generalized state and exact-result interface for both backends. Never flatten multi-color ownership into those two bitboards as an evaluation shortcut.

Initially, a general finite-horizon implementation can use core transitions and full keys. This provides a clear reference and shallow exact results before optimizing. Retain the current rule that failed horizons establish only finite lower bounds; exhausted budgets yield Unknown.

For a later compact backend, an exact option is three bits per playable cell: 24 × 3 = 72 board bits, plus turn and restriction = 76 bits, fitting in `u128`. Ownership and per-color totals live in its namespace; reserves follow from board counts. Alternatively use six color bitboards. Benchmark transition generation, canonicalization and browser memory before selecting the optimized layout. Wider proof keys and serialization need new versions; the old target-bit layout cannot be reused.

Generate a variable number of successors or size storage to validated configuration limits. With three available colors, the empty-board maximum is 31 legal moves, versus 15 today. If advanced settings allow a player more than three colors, 31 is not a universal capacity.

Update all color-dependent reasoning, including the independent core proof verifier:

- Terminal wins test each color separately and then map to owners.
- Insufficient material means every color owned by the target has fewer than three total cubies, not that the target has fewer than three cubies in aggregate.
- Earliest-win lower bounds consider the minimum required drops for any individually viable owned color. Already placed cubies of different colors cannot be combined into a three-piece material bound.
- Move ordering scores monochromatic threats. Other colors owned by the same player also block a candidate monochromatic line. Ordering is heuristic only and must never become an exact evaluation.
- Keep draw safety closure and independent verification; no horizon cutoff or encountered cycle is itself a draw proof.

### Expected difficulty and useful symmetries

More colors increase both branching and distinct board states, despite the preset using only 18 pieces. For illustration, for nine fixed occupied cells of one player, splitting them into three labeled colors with three each gives `9! / (3!³) = 1,680` assignments. Splitting both players gives 2,822,400 assignments before legality, termination and symmetry reductions. This is a combinatorial illustration, not a measured reachable-state count or runtime forecast.

Keep the eight existing spatial symmetries, transforming the restriction with the board. Colors have no intrinsic rule differences, so consistent color relabeling is another useful equivalence: transform board labels, ownership and inventories together. Within a fixed three-each namespace, permutations of the three colors owned by each player provide up to 3! × 3! = 36 equivalent labelings; actual reduction depends on the position. Avoid naively trying every permutation at every node without measuring its cost. Unequal totals require special care: only namespace-preserving permutations can be used without changing the namespace too.

Player swapping remains valid as a full relabeling of ownership, turn, inventories as represented, and winner. The current one-to-one board-color swap is insufficient for general unequal color sets. Initially omit this optimization from the general path if needed, then add it with transition-equivalence tests. Preserve existing single-color player-swap behavior.

Expect immediate wins, short tactics and some small/endgame positions to be practical evaluation targets. A small board population or exhausted reserves alone does not guarantee an easy solve: manipulation moves still create cycles. No claim about exact opening solvability is justified without a prototype and measurements.

## Background work and honest results

The existing worker already publishes partial exact results, retains bounds, rotates among requested moves and offers continuation. Currently it permits eight automatic batches of 250,000 horizon calls; continuation raises retained-bound capacity up to one million. Those are existing controls, not suitable multi-color defaults established by measurement.

For the generalized path:

- Prioritize current-position moves and shallow tactics; fairly distribute work across colors and unresolved rows.
- Make search resumable in short chunks and let a new position take priority between chunks. Cooperative cancellation requires returning to the worker event loop; a request ID alone does not interrupt synchronous search.
- Budget the whole batch, including safety closure, exact-value extraction, graph expansion and serialization. Today the recursive call cap does not bound all of that work.
- Track approximate bytes as well as entry counts: larger keys and additional metadata change memory cost. Use bounded retention and report limits accurately.
- Retain completed bounds for the matching namespace; stale responses may add compatible cached values but must not overwrite current task status.
- Show Unknown with separate searching/paused status, and provide pause/resume. Continued background work remains optional; playing never waits for evaluation.

Do not interpret a heuristic score as a proved win/draw. If approximate suggestions are added later, expose them separately from exact results. The first release can retain Unknown for all unresolved multi-color moves.

## Compatibility and regression protection

Do not simply change serialized Rust structs or increment the global rules constant and lose every bundled asset. Keep explicit old wire structs/readers for raw legacy positions, `RCGPOS01` snapshots, `RCGEVAL1` tables and existing proof formats. Translate validated single-color data into generalized states/keys, re-canonicalizing when necessary. Preserve specialized proof decoding/verification or provide an explicit verified migration. Continue rejecting the obsolete hash-only evaluation fixture.

Version new position, key and proof representations separately from the unchanged single-color game semantics. New multi-color data must never be interpreted by old two-color decoders. Detect conflicting converted cache entries. Keep original bundled artifacts usable without requiring an expensive regeneration as part of the gameplay release.

Regression gates:

1. Existing complete small-game values, all 15 `(12,12)` opening labels, its 11-ply optimal game, `(3,1)`'s nine-ply game, and `(3,3)` opening draws remain unchanged.
2. Existing shipped tables and proofs still load and verify; old saved positions retain ownership, inventories, turn, restriction and restart behavior.
3. Compare the old optimized backend and generalized reference on reachable single-color states: legal successors, outcomes, keys through adapters, and exact values.
4. Add multi-color cases for mixed-color non-wins; a monochromatic win in every supported color; multiple winning colors of one owner; both-owner draws; stock exhaustion; illegal opponent colors; undo/restart/import; and color ownership/key isolation.
5. Compare generalized packed transitions with core and bounded results with complete tiny graphs. Test color permutations and player relabeling for successor/outcome equivalence.
6. Extend browser smoke tests for selection, all six colors, previews, move rows, saved games and rapid switching between presets while evaluation runs. Confirm stale results and exhausted budgets cannot freeze play or invent values.

## Delivery sequence

1. **Compatibility foundation.** Capture legacy fixtures and baseline results; separate old wire formats from live model structs and define backend adapters. Establish two-color correctness and performance checks.
2. **Core and playable UI.** Generalize ownership, stocks, keys, outcomes and persistence; add presets, selectors and rendering. Route supported single-color games to the existing search. Publish terminal results for multi-color games and leave unresolved values Unknown. Exit criterion: complete multi-color play, undo, restart and import/export while all legacy regression gates pass.
3. **General exact evaluation.** Enable bounded core-based search and small retrograde solves; update proof checks and scheduling. Exit criterion: exact results agree with tiny complete multi-color graphs, partial results are useful, and interruptions preserve correctness and responsiveness.
4. **Measured optimization.** Add compact color-aware states, safe relabeling canonicalization and better ordering. Measure nodes/calls per second, cache hit rate, memory, batch latency and solved-move coverage in WASM as well as native builds. Include the requested opening and representative midgames. Retain the old fast path until equivalent correctness and acceptable speed are demonstrated.
5. **Optional larger studies.** CLI configuration for multi-color precomputation/proof verification, deeper opening analysis and optional new bundles. Neither a full table nor a solved opening blocks release.

The largest correctness risks are state/cache aliasing, silently invalidated old artifacts, and proof shortcuts that aggregate colors. The largest performance uncertainty is exact evaluation coverage. Keeping those separate from ordinary gameplay gives a useful first release without weakening the existing games.
