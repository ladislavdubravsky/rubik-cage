# Multi-color evaluation and a practical AI

Research and experiments: 2026-09-29. The target is `((4,4,4),(4,4,4))`: two players, each owning three colors and four cubies of each color.

Useful exact evaluation is already possible in this game, including nontrivial reachable tactics. The empty opening is still unresolved in the bounded tests below. A practical AI need not wait for an opening solution: the recommended next implementation is iterative-deepening alpha-beta with tactical checks, a color-aware heuristic, and exact endgame analysis where available. This recommendation is an inference from the local experiments and research in related games, not a measured playing-strength claim.

## Rule boundary discovered during research

The licensed 2019 John Adams/Rubik’s leaflet specifies 24 cubies, three colors per player, and a draw when the final cubie is placed without a winner. A player who individually runs out of cubies can still rotate or flip while the game continues. See [the manufacturer-authored leaflet, mirrored PDF, page 2](https://manuals.plus/m/81e7c0f96397fd6c820c056184c86d23c06689b16e3b5a84ede25810cfb59942.pdf).

The simulator instead allows continued manipulation after **all** reserves are exhausted, including a full board. Its simultaneous-opposing-lines convention is also an explicit simulator rule. Existing single-color results use these rules. This work preserves them.

Consequently, the full-board results below apply to the simulator's continuation variant. They are not physical-game endgames under the leaflet's stopping rule. The three tactical proofs below finish before reserves can run out and survive that particular rule difference.

Before training or presenting an AI as playing the physical rules, add a separate stopping-rule option with a distinct rule identity in keys, snapshots and caches. Do not silently reinterpret existing saved games or tables. The physical stopping rule still permits cycles before the final drop, since players can keep manipulating.

## Exact results obtained

Native release experiments used the existing compact general solver, a maximum horizon of 12, at most 1,000,000 proof records and 256 MiB estimated search memory. Opening/midgame trials were limited to 12 seconds each. The configured horizon is a limit: it does **not** mean every query completed through depth 12. Measurements are single local runs, sometimes concurrent with compilation, not stable performance benchmarks.

| Position | Exact result | Time searching | Independent verification | Displayed moves solved |
| --- | --- | ---: | ---: | ---: |
| Empty `((3,3,3),(3,3,3))` | Unknown | 12.03 s | — | 0/31 |
| Empty `((4,4,4),(4,4,4))` | Unknown | 12.01 s | — | 0/31 |
| Sample four-each midgame | Unknown | 12.01 s | — | 0/31 |
| Reachable four-each tactic after 10 moves | P1 wins in 3 | 40 ms | 52 ms | 1/25 |
| Reachable four-each tactic after 20 moves | P1 wins in 5 | 40 ms | 42 ms | 22/25 |
| Reachable four-each tactic after 21 moves | P2 loses in 4 | 6 ms | 4 ms | 17/17 |

The four-each opening retained 132,837 completed proof bounds and about 16.3 MiB estimated search memory, without proving an exact opening value. More time may help; these tests do not establish that an exact opening solution is impossible. The tactical examples were selected from deterministic legal random play, not an optimal line from the opening. They have no immediate winning move for the player to move. They leave respectively 14, 9 and 8 cubies in reserve, more than their proven distances.

Importable snapshots and complete legal histories are in [the fixture directory](../tests/fixtures/multicolor-exact/README.md). Raw data is in [multicolor-exact.csv](benchmarks/multicolor-exact.csv). Reproduce with:

```sh
cargo run --release --example explore_multicolor -- \
  --seconds 12 --discover-seconds 20 --csv /tmp/multicolor-exact.csv \
  --fixtures /tmp/multicolor-exact-fixtures
```

## A tractable exact endgame class

Under the simulator's continuation rules, a full 24-cell cage has no legal drops, and gravity is inert. Let `R0`, `R1`, `R2` be quarter rotations of the three layers, and `F` a flip. Layer rotations commute, each has order four, and a flip has order two. Conjugating a rotation by a flip reverses its direction and swaps the upper/lower layers. Every reachable board is therefore represented by:

`R0^a R1^b R2^c F^d`, with `a,b,c ∈ {0,1,2,3}` and `d ∈ {0,1}`.

There are at most `4³ × 2 = 128` board arrangements. Including two turns and eight inverse-restriction codes gives at most **2,048 raw states**, before symmetry reduction. Terminal states only shrink the graph. This bound is our derivation from the simulator's move rules, not a result taken from a paper.

Five reachable four-each full boards had only **365–449 canonical states** and **1,855–2,695 edges**. Complete native solves took **9–44 ms**, with separate verification of all graph equations. They included draws and positions with forced wins/losses several plies away. A saved six-ply loss has 392 states and 2,064 edges. See [full-board measurements](benchmarks/multicolor-full-board.csv).

The production worker now enables and prioritizes its incremental complete-graph solver for full boards, alongside its existing tiny-inventory cases. The graph can finish even if the finite-horizon proof-record allowance is exhausted. Existing work/memory limits, partial exact publication and independent verification remain intact. No new precomputed bundle is necessary.

```sh
cargo run --release --example explore_multicolor -- \
  --position tests/fixtures/multicolor-exact/full-board-loss-in-6.rcg
```

This is an on-demand solution of a given board's small orbit, not a table of every possible 24-piece arrangement. The bound does **not** apply to 23 pieces: a hole allows gravity to move cubies between layers. A future exact extension is an exhausted-stock board whose eight columns all have the same height `h`; the corresponding bound is `32 × 4^h` states. That extension is not enabled here.

## Research: which algorithms fit?

I did not find a separate published multi-color Rubik’s Cage solver in the searches performed. The recommendations below transfer established methods from other two-player games; the game's own tactical and cycle behavior must decide their value experimentally.

| Method | Evidence and implication |
| --- | --- |
| Iterative deepening, alpha-beta, transposition tables, tactical leaf search | The [official Stockfish search implementation](https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp) is a concrete architecture to study. The reusable ideas are bounded deepening and search reuse; chess-specific pruning assumptions do not automatically transfer. |
| Proof-number search | [Kishimoto and Müller (2008)](https://link.springer.com/chapter/10.1007/978-3-540-87608-3_14) show that ordinary depth-first proof-number search is incomplete on cyclic graphs. [Kishimoto (2010)](https://ojs.aaai.org/index.php/AAAI/article/view/7534) develops techniques for loops and proof-number estimation. A naïve cyclic df-pn implementation is unsuitable here. |
| MCTS with exact tactical propagation | [Winands, Björnsson and Saito (2008)](https://dke.maastrichtuniversity.nl/m.winands/documents/uctloa.pdf) separate proven results from simulation averages in MCTS-Solver. Their original implementation explicitly excludes proven draws. They also demonstrate that ordinary random-play MCTS can prefer a forced loss when samples miss its narrow refutation. |
| MCTS plus shallow minimax | [Baier and Winands (2015)](https://dke.maastrichtuniversity.nl/m.winands/documents/mcts-minimax_hybrids_final.pdf) examine tactical minimax in expansion, rollouts and backups across several games. Benefits depend on the game and time budget; making each rollout stronger but slower is not automatically an improvement. |
| Learned policy/value and self-play | [Silver et al. (2017)](https://arxiv.org/abs/1712.01815) establish policy/value learning combined with search in chess, shogi and Go. This is a later option here; it supplies approximate guidance, not a proof of the opening. |

## Proposed first AI

Use **iterative-deepening alpha-beta/negamax**, with a legal fallback move and the last completed iteration retained when its time expires. Reuse compact transitions and valid spatial/color symmetries. Keep an approximate transposition table separate from `EvaluationMap`, with searched depth, lower/upper/exact-at-that-depth flag, age and rule namespace. An alpha-beta bound on heuristic minimax is not an exact game-theoretic bound.

Search priorities:

1. Use proved terminal, material and applicable endgame values first.
2. Enumerate every actual legal move to find immediate wins, including rotations and flips. Preserve simultaneous-line draws.
3. Check opponent replies; when a safe alternative exists, reject moves allowing an immediate opposing win.
4. Deepen the remaining choices with move ordering and bounded extensions at tactically unstable leaves.

A pair of visual threats is not a verified fork: one manipulation can remove both, win for the opponent, or produce a draw. A forcing-threat search must check **all** legal defenses. Empty-layer rotations remain real tempo moves because they change turn and inverse restriction. Start without null-move pruning, chess-style captures-only quiescence, or unverified “obviously irrelevant” move pruning.

The approximate leaf evaluator should be antisymmetric: `E(s,p) = P(s,p) − P(s,1−p)`. The existing move-ordering function has different friendly/enemy weights and must not be copied unchanged into negamax.

Candidate features, to tune rather than treat as established knowledge:

| Feature | Intended role |
| --- | --- |
| Immediate legal wins and safe replies | Tactical search determines these; stronger than geometric patterns |
| Two cubies of one color with a legally droppable matching third | Strong positional threat; requires that exact color in reserve |
| Other unblocked same-color pairs | Weaker potential: the empty cell may require support or manipulation |
| Single-color open lines with sufficient material | Longer-term potential, scored separately by color |
| Multiple independent threat colors | Reward alternative plans without counting the same completion many times |
| Available manipulations and their safe continuations | Let shallow search assess whether a rotation/flip preserves or destroys a plan |

Another friendly color blocks a monochromatic line just as an opposing color does. Cubies of a color with fewer than three total pieces cannot win, but can still block or support other colors. Reserve exhaustion does not eliminate manipulation-based threats. “More pieces placed” alone is therefore a poor evaluation.

A cheap initial feature weighting could be 12 for a directly completable pair, 3 for another unblocked pair, and 1 for a viable open single, with support/stock discounts. Those are **untuned starting values**, not a claim of playing strength or win probability. Search, especially manipulation responses, should dominate this static estimate.

Compact canonicalization also changes orientation and color labels. Map a recommended compact child back through the original state's legal moves before displaying or executing it.

## Further exact progress before large-scale learning

- **Publish a verified winner before the exact distance is known.** An upper proof can establish “P1 wins within ≤8 plies” while the shortest distance is unresolved. Add a separate result type rather than putting an upper bound in the existing exact `plies` field.
- **Try proof-number priorities within the finite-horizon search.** A key `(position, target, remaining horizon)` has decreasing horizon, so the proof problem is acyclic even when board positions repeat. This may focus work better; benchmark it against the current round-robin solver and keep the independent verifier.
- **Reuse closed endgame solutions as boundaries.** On-demand complete graphs can help earlier search, but their certificates and distances must remain checkable; a nearly full board is not automatically a small graph.
- **Investigate closed safety certificates.** A verified non-losing strategy needs a safe successor at the defending player's choices and all successors at the opponent's choices. Cyclic closure can prove non-loss without a complete global table. A failed horizon cannot.

Then compare a tactical MCTS/MCTS-Solver player against the alpha-beta baseline under equal elapsed time. Use immediate-win/evasion checks in simulations, exact-result propagation, tree reuse and bounded rollouts with a heuristic cutoff. A rollout cap or repeated path is an approximate adjudication, never an entry in the exact draw cache. Repetition-dependent scores must not be cached as unconditional state values.

Learning should start modestly: fit feature weights or a small evaluator using exact tactical/endgame labels and games against several baselines. Split training/validation by canonical position to avoid symmetry leakage. Policy/value self-play is reasonable only after the game rules and evaluation harness are stable.

## Product and validation plan

Keep “Exact result: Unknown” alongside a separate “Suggested move; depth 5” or similar analysis. Do not turn a heuristic score or rollout average into an uncalibrated winning percentage. Keep rule variant, position key, evaluator version, completed depth/work and principal variation in the approximate result.

Before offering an AI player:

- Pass immediate-win and safe-defense fixtures involving drops, rotations, flips, gravity and simultaneous lines.
- Measure move quality against exact tiny-game and full-board oracles under the appropriate rule variant.
- Compare random, tactical-greedy, alpha-beta and tactical-MCTS players at equal budgets such as 100 ms, 1 s and 10 s per move.
- Use fixed reachable starts, paired player seats and color permutations; report uncertainty across starting positions.
- Report repeated games and move-cap adjudications separately from proved draws.
- Measure browser responsiveness and retain every two-color compatibility regression.

Validation for this change: all 75 native release tests passed, including independent proof checks and a full-board solve with zero finite-horizon proof-record allowance. The release WASM build and browser smoke suite passed, covering the five-ply tactic, full-board draw/loss and existing two-color workflows. The single-color audit retained the exact `(12,12)` result and its 21,072 calls / 4,874 bound records.

The immediate deliverables here are the exact-search improvement, reproducible proved fixtures and this researched AI plan. An autonomous approximate player has not been added to the UI.
