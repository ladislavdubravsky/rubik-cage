# Exact multicolor successes under the simulator rules

**Rules scope:** These results use the existing simulator rules, which allow rotation/flip play after all cubies have been placed. The [2019 John Adams manufacturer instructions](https://manuals.plus/m/81e7c0f96397fd6c820c056184c86d23c06689b16e3b5a84ede25810cfb59942.pdf) end the physical game in a draw when the last cubie is placed without a winning line. Consequently, the full-board continuation graphs and their six-ply win are simulator-variant results; they are not official physical-game endgames. No terminal rule is changed by these experiments.

Every fixture starts from the empty `((4,4,4),(4,4,4))` game: Player 1 owns White, Blue and Green; Player 2 owns Yellow, Red and Orange. Each color starts with four cubies. The adjacent `.txt` file lists a legal move history from that opening. All intermediate states and saved positions are nonterminal. Coordinates and rotation directions use the Rust core `Move` representation.

The `.rcg` files are ordinary `RCGPOS02` snapshots, importable with the app’s Import position control. Distances count individual moves, including rotations and flips. “Loss” is from the player to move's perspective.

| Snapshot | Turn | Exact result | History length | Cubies in reserve | Verification |
| --- | --- | --- | --- | --- | --- |
| `full-board-draw.rcg` | Player 1 | Draw | 24 | 0 | Complete core graph: 393 states, 2,119 edges |
| `full-board-loss-in-6.rcg` | Player 2 | Player 1 wins in 6 | 25 | 0 | Complete core graph: 392 states, 2,064 edges |
| `tactic-win-in-3.rcg` | Player 1 | Player 1 wins in 3 | 10 | 14 | General bounds checked through core transitions |
| `tactic-win-in-5.rcg` | Player 1 | Player 1 wins in 5 | 20 | 9 | General bounds checked through core transitions |
| `tactic-loss-in-4.rcg` | Player 2 | Player 1 wins in 4 | 21 | 8 | General bounds checked through core transitions |

For all three tactical fixtures, the proven winning distance is smaller than the number of cubies in reserve. Since each move places at most one cubie, their winning horizons finish before the supply could be exhausted. This particular rule difference therefore cannot affect those bounded proofs.

The tactical fixtures have no immediate winning move for the player to move. They were selected from deterministic legal random play, not optimal opening play. They establish that useful exact analysis is possible inside the requested inventory; they do not solve its empty opening.

Reproduce the native experiments:

```sh
cargo run --release --example explore_multicolor -- \
  --seconds 12 --discover-seconds 20 --csv /tmp/multicolor-exact.csv \
  --fixtures /tmp/multicolor-exact-fixtures
cargo run --release --example explore_multicolor -- \
  --full-board --fixtures /tmp/multicolor-exact-fixtures
cargo run --release --example explore_multicolor -- \
  --position tests/fixtures/multicolor-exact/full-board-loss-in-6.rcg
```

The generator uses fixed xorshift seeds. Tactical *selection* also has a per-candidate time limit, so hardware can affect which candidates finish proving; the checked-in snapshots and move histories fix the selected cases independently of timing. `sample-67`, `sample-25`, and `sample-18` are respectively the win-in-3, win-in-5, and loss-in-4 measurements in [the native CSV](../../../docs/benchmarks/multicolor-exact.csv). Full-board generation found the draw on its first attempt, and the six-ply loss is the `Up / Clockwise` child of `full-444-4`.

`max_horizon` in the CSV is a search limit, not a claim that every shallower horizon completed. Unknown means the allotted search did not prove an exact value. Timing is a single native release run, with background work possible; estimated bytes are the solver's conservative accounting, not measured resident memory. The full-board CSV measures direct complete retrograde solving under the simulator continuation variant; bounded general worker scheduling has additional overhead.
