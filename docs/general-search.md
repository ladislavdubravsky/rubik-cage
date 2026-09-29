# General exact evaluation

Multi-color games use `search/general.rs`, a compact color-aware solver with an independent core proof verifier. One-color-per-player games still use the existing packed solver and bundled tables. General proof records have no disk format yet; legacy tables and proofs remain unchanged.

## What is proved

For each requested position (the displayed root and its distinct children, sharing queries across equivalent colors), the solver searches increasing horizons for each player. The target chooses one winning successor; the opponent must have only winning successors. Completed queries retain upper and lower bounds. A win and its distance are published only when those bounds meet. Interrupted queries keep their explicit stacks; unfinished ancestors never become proof records.

Material bounds count each exact color separately. A player with two White and two Blue cubies cannot form a line, despite owning four cubies. If neither player has three cubies of any owned color, the position is an exact draw. Otherwise, a horizon failure, repeated position, memory limit or elapsed work budget leaves the value Unknown.

`Proof::verify` checks completed bounds using ordinary core transitions. Dependencies decrease the horizon, so cyclic claims cannot justify finite wins. It is intended for tests/offline verification, not an unbounded phase in a browser batch.

For inventories of at most five total cubies, and for completely full 24-cell boards, `general_graph.rs` also expands a complete reachable graph incrementally. Full boards have at most 2,048 raw states under the simulator’s continuation rules, and their graph gets priority over finite-horizon work. It retains its frontier between batches and uses distance-ordered retrograde propagation. Only after complete expansion and propagation can unresolved nodes be classified as draws. If the graph exceeds 4,096 states or 65,536 edges, it is abandoned without publishing values or repeatedly restarting. Larger graphs can be checked with the existing offline retrograde reference.

## Background work and limits

Each general worker batch performs at most 4,000 small search/graph steps, checking a 40 ms time target between steps. One step can generate all legal successors (up to 55), so the time target is cooperative, not a hard real-time guarantee. Replies contain only requested exact values, at most 56 entries. There is no whole-proof scan or safety-closure pass in the batch.

Requested moves share search steps round-robin. A changed position replaces suspended root tasks and its small graph, while retaining completed bounds if ownership and all six total inventories match. The classic and general search instances are separate. Complete state keys prevent cached results from crossing incompatible games.

There are 32 automatic batches per allowance and up to four allowances. Continue search raises the limits:

| Allowance | Bound records | Estimated search memory | Horizon |
| --- | ---: | ---: | ---: |
| Initial | 25,000 | 16 MiB | 6 |
| Second | 50,000 | 32 MiB | 8 |
| Third | 75,000 | 48 MiB | 10 |
| Fourth | 100,000 | 64 MiB | 12 |

Memory accounting conservatively estimates hash-table slack, suspended stacks and graph storage; it is not a measurement or limit on total browser memory. Cached UI values and the separate classic backend also consume memory.

Pause search lets the current batch finish and prevents further automatic batches for that position. Continue resumes a manually paused allowance without raising its limits; continuing after a resource pause grants the next allowance. Move selection, undo, restart and imports remain available during computation. A prior position's response can contribute keyed exact values, but its paused status does not replace the current position's status.

## Validation and remaining work

Regression coverage compares interrupted horizon wins and complete incremental graphs against `retrograde::solve`, independently verifies proofs/graph values, exercises corrupted proofs and resource exhaustion, and checks per-color material and namespace isolation. Browser coverage includes nonterminal multi-color wins, material draws, manual pause/resume, and switching to the unchanged classic opening while work is pending.

The three-colors-each opening is not solved by this implementation. Short tactics and small games are useful targets; difficult positions remain Unknown. Step 4 adds compact states, safe color relabeling and monochromatic move ordering; see [measurements and correctness checks](search-performance.md). General proof/table serialization, larger draw certificates and larger offline studies remain optional follow-up work. See [four-per-color experiments and the researched AI plan](multicolor-ai-research.md), including the physical-game stopping-rule distinction.

The [first AI player](ai-player.md) uses a separate approximate search and result type. It can suggest or play moves while exact evaluation remains Unknown. Heuristic scores and alpha-beta bounds never enter the exact result map; completed exact proofs can update an ongoing AI analysis.
