# Frozen single-color compatibility fixtures

Captured using the unmodified encoders at commit
`8c85584da7270994f81a4987c79e15cb04cb32a9`, before the multi-color compatibility refactor.
Do not regenerate these from current structs: they are migration inputs, not snapshots
that should change when the model changes.

- `rotation.position.bin`: `RCGPOS01`; P1 White, P2 Green; initial stocks 20/4;
  White drops at (0,0), Green at (2,1), then Down rotates clockwise. Remaining
  stocks 19/3; P2 to move; immediate counterclockwise Down rotation forbidden.
  The stock of 20 deliberately exceeds the UI limit while satisfying legacy core rules.
- `flip.position.bin`: the preceding position after Flip; P1 to move; Flip forbidden.
- `*.raw.bin`: corresponding original unversioned `GameState`, with the derived
  Zobrist hash deliberately set to 123. Import must rebuild it.
- `horizon.proof-v1.bin`: original horizon-only `RCGPRF01` wire layout, generated
  by `Search::exact(GameState::new(3, 0), 9)`; certifies a P1 win in five plies.
- `horizon.proof-v2.bin`: the same search after `close_safety`, saved by the original
  `RCGPRF02` encoder. The shipped full-inventory proof supplies nonempty safety claims.

The existing `assets/eval-v1.bin`, `assets/eval-3-1.bin` and
`assets/eval-v1.proof.bin` are also fixed regression fixtures. The integration suite
checks table byte round trips, all full-inventory entries against the independently
verified proof, all opening move values and both saved-position formats. The obsolete
`assets/eval.bin` remains a rejection fixture.

`SHA256SUMS` records the small fixture bytes; run `sha256sum -c SHA256SUMS` here.
