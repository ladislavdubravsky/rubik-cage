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

A player cannot undo the opponent's immediate previous move. Empty-layer rotations and other moves that leave the board unchanged are still legal and update this restriction. A move producing lines for both players is a draw; indefinite play is also evaluated as a draw.

The exact solver uses retrograde analysis of the finite game graph, including the previous-move restriction. Small games are verified against an independent fixed-point solver. The previous claims about solving every inventory, including `(12,12)`, came from an unsound search and are withdrawn. Under the corrected rules, `(3,1)` is a P1 win in 9 plies and `(3,2)` is drawn. A ply is one player's move.

## Webapp build

Install [webassembly target and trunk](https://yew.rs/docs/getting-started/introduction#install-webassembly-target).

```
trunk build --release
```

or `trunk serve` to serve with hot reloading.

### Precomputing evaluations

[eval-v1.bin](./assets/eval-v1.bin) is a verified complete `(3,1)` table containing 2,668 exact positions. Keys include the board, turn, inventories, colors and the immediate-undo restriction, canonicalized together. The format records rules, key and solver versions and distinguishes complete from filtered tables. The old `assets/eval.bin` is retained only as a rejected legacy regression fixture; it is never loaded by the app.

Generate, independently verify, then optionally filter a table:

```sh
cargo run --release --bin evaluator -- evaluate 3 1 assets/eval-v1.bin
cargo run --release --bin evaluator -- verify assets/eval-v1.bin
cargo run --release --bin evaluator -- filter assets/eval-v1.bin /tmp/eval-subset.bin 3
```

Native searches default to 100,000 states and 1,500,000 edges. Override these with `--max-states` and `--max-edges` on `evaluate`. Reaching a limit returns an error and writes no table. Filtering preserves metadata; missing entries mean unknown. Full minimax verification of a subset requires its complete source table.

The browser retains the `(12,12)` starting game, but its initial evaluation is currently **unknown**: each background request is bounded to 20,000 states and 300,000 edges. The app displays exact results only after a complete solve (or a compatible cache hit); exceeding the budget never becomes a draw. Larger precomputation and graph-storage/component optimizations remain future work. A single shared worker reuses exact results between requests.

Move-row distances include the selected move: an immediate win displays “Win in 1.” Imported legacy positions are validated and their derived identity is rebuilt; newly exported positions use a versioned format. Restart preserves the imported game's initial inventories and colors.

## Playing with core logic

Run `cargo test --all-targets` for model, symmetry, graph, cache and migration tests. The production solver is in `src/search/retrograde.rs`; historical `naive.rs` is not compiled.

```sh
cargo run --release --example audit_search -- 3 1
cargo build --release --target wasm32-unknown-unknown --bin app --bin worker
```

For the browser regression, serve a Trunk build and run `node scripts/browser_smoke.mjs http://127.0.0.1:8080/rubik-cage/` (Node 22+ and Google Chrome; `CHROME` can select another Chromium binary). This exercises worker reuse, the original move sequence, an optimal 9-ply game, unknown results at limits, saved-position import, undo/restart, and simultaneous-line termination.

See [the investigation](docs/search-investigation.md), [redesign plan](docs/search-redesign-plan.md), and [implementation notes](docs/search-implementation.md).
