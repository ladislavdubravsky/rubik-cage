# Single-color compatibility foundation

The first multi-color implementation step isolates existing persisted formats and
establishes regression gates. Step 2 adds multi-color gameplay while preserving
the default settings, bundled assets, and specialized 52-bit search backend.

## Boundaries to retain during the core migration

- `src/compat.rs` owns frozen wire layouts for original raw positions, `RCGPOS01`,
  and the keys, colors, moves, inventories and evaluations embedded in other files.
  Conversion validates untrusted input and rebuilds derived state identity. Legacy
  key canonicalization is frozen separately from live key canonicalization.
- `src/search/cache.rs` reads/writes `RCGEVAL1` using those wire types, validates
  converted entries, and rejects duplicate keys and obsolete hash-only files.
- `src/search/bounded.rs` reads both `RCGPRF01` and `RCGPRF02` through explicit wire
  tuples, validates every proof against core transitions, and rejects duplicate
  bound/safety claims. `Proof::encode/decode` are the persistence API; do not
  serialize the live `Proof` struct directly.
- `src/search/single_color.rs` is the checked adapter between the live model and
  the specialized search. `Space::try_encode` reports unsupported/mismatched
  input as an error. `Search::exact` uses it rather than panicking on a mismatched
  nonterminal game. Packed move generation and finite-horizon search are unchanged.

When introducing the new core model, update these conversions explicitly. Reject
multi-color configurations at the specialized solver boundary; route them to the
new backend. Add new wire formats instead of modifying the frozen ones. Keep the
old rules identifier independent of future format/rules identifiers. Never merge
multi-color positions by player occupancy alone.

## Regression gates

`tests/fixtures/single-color-v1` contains fixed bytes captured before the refactor,
with provenance, expected semantics and SHA-256 checksums. The raw fixtures have
stale hashes and stocks beyond the settings panel's limit, protecting existing
import behavior. Existing bundled tables/proofs are also fixed test inputs.

`tests/single_color_compat.rs` checks:

- old raw and versioned positions, including nondefault colors, stocks, turn and
  immediate-inverse restrictions;
- truncation, trailing bytes, unknown versions and rejected obsolete caches;
- both proof versions and deterministic current proof encoding;
- all 122,727 full-inventory table values against the independently verified proof;
- full-inventory table byte round-trip, all 15 opening move values, and the
  `(3,1)` complete table's values, roots and coverage;
- the specialized solver's transitions against core for all 30 distinct ordered
  color pairs, and error returns for invalid/mismatched configurations.

The `(3,1)` asset's historical generator string is `exact-retrograde-v1`; the
pre-refactor and current writers both emit `verified-exact-v1`. Its regression
therefore compares game data rather than requiring that metadata to be identical.
The full-inventory asset already has the current generator string.

Existing model, graph, bounded-proof and worker tests remain in place, including
complete small-game comparisons, `(3,3)` draws and continuation. Browser smoke
coverage protects all opening labels, optimal 11-ply and nine-ply games, imports,
undo/restart, settings, switching inventories and background worker reuse.

Run:

```sh
cargo test --release --locked --all-targets
cargo run --release --locked --example audit_compat
trunk build --release --locked
node scripts/browser_smoke.mjs http://127.0.0.1:8080/rubik-cage/
```

The performance audit samples bundled-table loading and a cache-free `(12,12)`
opening five times. Compare release builds on the same machine without competing
builds or browser search. Wall-clock times are observations, not CI assertions.
The pre-refactor opening baseline is 21,072 recursive calls and 4,874 bound records,
with P1 winning in 11 plies; the compatibility implementation preserves those counts.

## Recorded step-1 validation

On 2026-09-28, all 53 native tests passed (47 unit tests and six compatibility
integration tests), the release WASM app/worker built, and the existing browser
smoke suite passed. The bundled assets were not regenerated.

Sequential five-sample native release audits on this development machine compared
commit `8c85584da7270994f81a4987c79e15cb04cb32a9` with the compatibility implementation:

| Measurement | Original median | Compatibility median |
| --- | --- | --- |
| Load full-inventory table | 403 ms | 470 ms |
| Cache-free opening search | 41 ms | 34 ms |
| Opening recursive calls | 21,072 | 21,072 |
| Opening retained bounds | 4,874 | 4,874 |

These are observational timings with overlapping sample ranges, not a claimed
search speedup. Explicit wire conversion adds some table-loading work; no extra
search nodes or bounds are needed. The browser regression confirms the existing
interaction and evaluation workflows still complete.


## Step-2 integration

The live state now stores exclusive color ownership and per-color reserves.
Legacy adapters explicitly translate the old player colors/stocks into that model;
legacy table/proof layouts remain unchanged. The specialized solver rejects any
configuration with more or fewer than one assigned color per player. An assigned
color still counts when its current inventory is zero, preventing accidental
cross-configuration cache reuse.

Multi-color saved positions use `RCGPOS02` with the existing frozen color/cage/move
wire types, six owner entries, six reserve counts, turn and previous move. Its
reader validates ownership, material, gravity, turn and restriction before use.
Single-color exports remain `RCGPOS01`, preserving the fixed export fixtures.
The browser skips unsupported search requests and still recognizes terminal move
values. Returning to a supported game reuses the existing worker and evaluations.


Step-2 native validation passed all 62 tests (49 unit tests, seven multi-color
integration tests and six fixed-format compatibility tests). The expanded browser
suite covers color selection and exhaustion, keyboard drops, previews, exact
immediate wins, multi-color export/import, same-owner and opposing-owner lines,
and switching configurations during pending search. The opening audit still uses
21,072 recursive calls and 4,874 bounds for the P1 win in 11 plies.
