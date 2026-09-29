# First AI player

The AI offers bounded advice and optional automatic turns for both single-color and multi-color games, including `((4,4,4),(4,4,4))`. It does not solve the opening. Exact move evaluations remain separate from its approximate recommendations.

## Playing

Both players start as Human. **Suggest move** analyzes the current position without playing; **Play suggested move** applies the result. The analysis displays a completed search depth, work count and a proposed continuation where available. These are search information, not a calibrated winning probability.

Choose Computer for either player and press **Start automatic turns** to play against it, or choose Computer for both for self-play. A normal move on a Human turn lets the computer reply. **Pause AI** stops further AI work and automatic moves. Undo, restart, import, changing the setup and overriding a Computer turn suspend automatic turns and discard old advice. Results from a previous board are never applied to the current one.

The time choices are 100 ms, 1 s and 10 s. They are approximate elapsed-time allowances, not difficulty ratings. Work runs in short batches on the same background worker as exact evaluation. Worker queueing may add latency. An incomplete immediate-threat screen can receive another 1.5 seconds, checked between worker replies, before the UI stops without playing. The last completed iteration is retained when time expires; automatic play requires a completed tactical screen.

## Search and result boundaries

The engine uses iterative-deepening alpha-beta/negamax over compact positions. Spatial symmetries and color permutations only combine positions when ownership and total stock permit them. It maps the chosen compact child back through the actual board's legal moves, including the original color and orientation.

At the root it checks every legal move for immediate wins and every opponent reply for immediate losses. This includes rotations and flips, with gravity and simultaneous-line draws handled by the core rules. If a safe option exists, the AI avoids a move that lets the opponent win immediately. A small tactical extension at leaves helps reduce horizon mistakes; it is bounded and does not claim to find every threat.

The positional score compares the two players symmetrically. It rewards open monochromatic pairs and singles, gives more weight to a pair with a legally droppable third cubie, and accounts for the exact color's reserves and available support. Another owned color blocks a line just as an opposing color does. Initial weights are hand chosen and have not been tuned for playing strength.

Known exact values take priority over heuristic estimates. The shared worker can use existing single-color values and completely solve full-board endgames before making a choice. Full boards are small complete graphs under the simulator's continuation rules. The [physical-rule distinction](multicolor-ai-research.md#rule-boundary-discovered-during-research) still applies; this AI does not change the rules.

The UI engine caps nominal depth at 6, with up to two tactical extension plies, immediate-win checks at leaves, and 25,000 approximate table slots. The approximate transposition table is separate from exact evaluation storage and bounded in size. Search depth, tactical extension and bound direction are part of its reuse rules. Finite search depth handles repeated positions: a repeated path is not published as an exact draw. A large positive score or an alpha-beta bound does not become an exact game-theoretic evaluation.

## Initial validation and measurements

On 2026-09-29, all 92 native release tests passed. Coverage includes independent core-rule tactical fixtures, symmetry/color mapping, interrupted iterations, transposition-table collisions, shallow minimax comparisons, exact and partial-oracle choices, worker sessions and UI cancellation. The release WASM build and browser smoke checks passed, including a human-versus-computer reply, a complete computer-versus-computer classic game, explicit pause, cancellation after imports/restarts/undo, and preservation of existing exact labels. The classic `(12,12)` audit retained its exact answer and 21,072 calls / 4,874 bound records.

The [native audit CSV](benchmarks/ai-baseline.csv) records single local runs on seven fixed four-each positions. Its limits are **depth 16 / 100,000 table slots**, rather than the UI's **6 / 25,000**; these measurements should not be read as browser depth guarantees. Search-step counts include tactical probes and frame returns, not only distinct positions.

| Four-each empty opening allowance | Completed nominal depth | Measured elapsed time |
| --- | ---: | ---: |
| 100 ms | 4 | 100.49 ms |
| 1 s | 6 | 1,000.31 ms |
| 10 s | 8 | 10,000.44 ms |

The opening remains exactly Unknown. At one second, the three saved tactical fixtures produced independently verified optimal choices: a Green drop for the three-ply win, an equator rotation for the five-ply win, and an upper-layer rotation delaying the four-ply loss. Those exact checks ran **after** timing and were not supplied to the AI. At 100 ms the five-ply fixture's chosen move remained unverified within the proof allowance; blank CSV oracle fields mean unverified, not a proved mistake.

A separate choice audit sampled 24 nonterminal positions, twelve evenly spaced sorted canonical keys from each of two independently verified full-board graphs. It ranks outcome first, then shortest win or longest loss. These full-board cases use the simulator continuation rules.

| Policy | Optimal outcome | Optimal outcome and distance |
| --- | ---: | ---: |
| Uniform random, analytical expectation | 17.17 / 24 | 8.67 / 24 |
| Immediate win, otherwise first safe move | 24 / 24 | 20 / 24 |
| Alpha-beta without oracle, 100 ms | 24 / 24 | 24 / 24 |
| Alpha-beta with exact local oracle | 24 / 24 | 24 / 24 |

This small selected corpus measures move choices, not game win rates or general playing strength. The random row is an expectation over legal choices, not sampled games. Timing depends on the machine and background work; exact-oracle preparation and later verification are outside the timed AI choice.

Reproduce with:

```sh
cargo test --release --locked --all-targets
cargo run --release --locked --example audit_ai -- \
  --opening-long --csv /tmp/ai-baseline.csv
```

## Limits and next steps

This is an initial hand-built player, not a measured strong opponent. A suggested continuation can change after deeper analysis. Large openings can have useful advice while every exact move label remains Unknown.

Next comparisons should use fixed reachable starting positions and paired player seats, measure tactical accuracy against exact oracles, and compare random, tactical-greedy and alpha-beta players at equal elapsed time. Games stopped by repetition or a move cap must be reported separately from proved draws. Tactical MCTS and learned evaluation remain later experiments; see the [research and longer-term plan](multicolor-ai-research.md).
