# Rubik's cage analysis

## Webapp build

Install [webassembly target and trunk](https://yew.rs/docs/getting-started/introduction#install-webassembly-target).

```
trunk build --release
```

or `trunk serve` to serve with hot reloading.

### Precomputing evaluations

[eval.bin](./assets/eval.bin) contains several MB of most useful precomputed evaluations for the `(12, 12)` game, with optimal numbers of moves to win/loss. This enables the webapp to display evaluations immediately. As the user reaches some remaining unevaluated positions in a game, these are fast enough to be computed on the fly in background web workers.

If you want to change how many precomputed evaluations are stored in the webapp binary, first run:
```
cargo run --release --bin evaluator evaluate 12 12 "eval_12_12_full.bin"
```
This calculates evaluations for all reachable `(12, 12)` game states and stores them (1.2 GB). Then:
```
cargo run --release --bin evaluator filter "eval_12_12_full.bin" "assets/eval.bin" <min_moves_to_wl>
```
which will filter for evaluations that take at least `min_moves_to_wl` to win/lose under optimal play. Alternatively, treat [`evaluator.rs`](./src/bin/evaluator.rs) as a scripting space and define your own logic.

## Playing with core logic

Run `cargo test` for core logic tests; explore and modify the tests to e.g. evaluate positions by searching the game tree, or play with the evaluator crate as a scripting pad.
