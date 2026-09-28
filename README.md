# Rubik's Cage Simulator

Rubik's cage is a multiplayer game that combines Rubik's cube with tic-tac-toe (or rather Connect 4, since gravity applies). The cage has a 3x3x3 layout with a blocked central column, leaving 24 usable slots into which colored cubies can be dropped. First player to get three in a line wins. Horizontal layers can be turned Rubik's cube style and cubies can enter either from the top or from the bottom (after the cage is flipped).

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

A player cannot undo the opponent's immediate previous move. Empty-layer rotations and other moves that leave the board unchanged are still legal and update this restriction. In this implementation, a move producing lines for both players is a draw; indefinite play is also evaluated as a draw.

Exact evaluation combines retrograde analysis of complete game graphs with finite-horizon minimax for larger games. Both include the previous-move restriction. Under the corrected rules, `(12,12)` is a **P1 win in 11 plies**, `(3,1)` is a P1 win in 9, and `(3,2)` is drawn. A ply is one player's move. The new `(12,12)` result has an independently checked proof; earlier claims from the unsound solver are superseded. This does not mean every reachable position has been evaluated.

## Webapp build

Install [webassembly target and trunk](https://yew.rs/docs/getting-started/introduction#install-webassembly-target).

```
trunk build --release
```

or `trunk serve` to serve with hot reloading.

### Precomputing evaluations

[eval-v1.bin](./assets/eval-v1.bin) contains **122,727 exact `(12,12)` evaluations**, with an independently checkable [proof](./assets/eval-v1.proof.bin). This is a subset: it includes every initial move, the opening winning strategy, and many alternatives; missing entries remain unknown. The browser also loads a complete `(3,1)` table from [eval-3-1.bin](./assets/eval-3-1.bin).

Keys include board, turn, inventories, colors and immediate-undo restriction, canonicalized together. The format records compatibility versions and coverage. The old `assets/eval.bin` is retained only as a rejected legacy regression fixture; the app never loads it.

Regenerate and certify the full-inventory subset:

```sh
cargo run --release --bin evaluator -- precompute 12 12 assets/eval-v1.bin \
  --proof assets/eval-v1.proof.bin --opening-plies 4 --max-horizon 18
cargo run --release --bin evaluator -- verify-proof assets/eval-v1.proof.bin \
  --table assets/eval-v1.bin
```

Generate a complete small-game table, independently verify it, then optionally filter it:

```sh
cargo run --release --bin evaluator -- evaluate 3 1 assets/eval-3-1.bin
cargo run --release --bin evaluator -- verify assets/eval-3-1.bin
cargo run --release --bin evaluator -- filter assets/eval-3-1.bin /tmp/eval-subset.bin 3
```

Complete graph searches default to 100,000 states / 1,500,000 edges; `--max-states` and `--max-edges` override these. Horizon precomputation has explicit position/call budgets and publishes exact distances from horizon proofs and draws from independently checked closed safety strategies. A finite search cutoff never implies a draw. See [the algorithm, proof format, budgets and measurements](docs/horizon-search.md).

The browser starts at `(12,12)` with all 15 opening moves evaluated:

| Opening move | Evaluation for P1, including that move |
| --- | --- |
| Any corner drop | Win in 11 |
| Any edge drop | Draw |
| Flip or any layer rotation | Loss in 12 |

The empty-board Flip and Rotate moves give P2 a win in 11 more plies; their immediate-inverse restrictions are included in the proofs. A single shared worker reuses exact values and finite-horizon bounds, with complete graph analysis available for draws. It can publish exact results for some moves while others remain unknown. Exhausting a budget never becomes a draw or an invented distance.

Move-row distances include the selected move: an immediate win displays “Win in 1.” Imported legacy positions are validated and their derived identity is rebuilt; newly exported positions use a versioned format. Restart preserves the imported game's initial inventories and colors.

## Playing with core logic

Run `cargo test --all-targets` for model, symmetry, graph, cache and migration tests. The production solvers are `src/search/retrograde.rs` and `src/search/bounded.rs`; historical `naive.rs` is not compiled.

```sh
cargo run --release --example audit_search -- 3 1
cargo build --release --target wasm32-unknown-unknown --bin app --bin worker
```

For the browser regression, serve a Trunk build and run `node scripts/browser_smoke.mjs http://127.0.0.1:8080/rubik-cage/` (Node 22+ and Google Chrome; `CHROME` can select another Chromium binary). This exercises worker reuse, the original move sequence, optimal 11-ply `(12,12)` and 9-ply `(3,1)` games, all opening labels and an edge-drop draw, saved-position import, undo/restart, and simultaneous-line termination.

See [the investigation](docs/search-investigation.md), [redesign plan](docs/search-redesign-plan.md), and [implementation notes](docs/search-implementation.md).
