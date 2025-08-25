# Rubik's Cage Simulator

Rubik's cage is a multiplayer game that combines Rubik's cube with tic-tac-toe (or rather Connect 4, since gravity applies). There is a 3x3x3 cage of empty slots into which colored cubies can be dropped. First player to get three in a line wins. Horizontal layers can be turned Rubik's cube style and cubies can enter either from the top or from the bottom (after the cage is flipped).

The puzzle is physically manufactured. You can check out a [vid](https://www.youtube.com/watch?v=xcPz_6yagjE) or the picture below.

<div align="center">
	<img src="https://cdn1.philibertnet.com/730774-thickbox_default/rubik-s-cage.jpg" alt="Rubik's Cage board game" width="300"/>
</div>

In this project we solve the puzzle for some classes of initial conditions and create a webapp simulator to explore the optimal moves. [Try it out!](https://ladislavdubravsky.github.io/rubik-cage/)

## Exact rules

For now we only consider two players, and each has cubies of one color only. If the players start with `m`, resp. `n` cubies, we call this a `(m, n)` game. Players take turns and on each turn the player has three available moves:

- drop a cubie of their color into one of the columns
- rotate one of the layers 90 degrees clockwise or counter-clockwise
- flip the cage upside down

A player cannot undo the opponent's immediate previous move.

We solved all `(m, n)` games for the cage. For example, a game of particular interest is the `(12, 12)` game (the cage has 24 available slots), which is a win for player 1 in 8 moves or less of optimal play.

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
