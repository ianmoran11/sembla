# Model-derived tiling evidence — 2026-07-28

## Implementation and structural safety

The evaluator now walks each eligible typed IR expression in its real
left-to-right evaluation order. The walk records the retained root width, peak
concurrent width, and node count. Binary nodes account for the left root while
the right subtree is evaluated and for any materialised `Int`-to-`Real`
coercion. This is a liveness bound, not a node-count-times-width estimate.

Transition tiles retain guard, hazard, claim-resource, and claim-key roots for
one transition. Count-view filters are reduced and dropped within their plan;
numeric-view roots remain live until the canonical ordered reduction. The
largest concurrent plan determines cache footprint, while node counts from all
eligible plans contribute to the work estimate.

The fixed cache budget is 32,768 bytes, matching PRD 0001's L1 assumption.
Derived row counts are clamped to 64–4,096 and rounded down to a multiple of 64.
`SEMBLA_EVAL_TILE_ROWS` remains an exact override. The default work threshold is
1,500,000 node-rows: `threading_spike`'s approximately seven-node guard measured
131,072 rows (917,504 node-rows) on the negative side and 262,144 rows
(1,835,008 node-rows) on the positive side.

Unit tests hand-check leaf, arithmetic, and deep-guard expression footprints,
plus three distinct model shapes: a mixed race/key transition model derives
37 bytes and 832 rows, demographic derives 33 bytes and 960 rows for its
transition phase, and the canary derives 41 bytes and 768 rows.

Changing the selection does not change operations or order. Tile boundaries
still depend only on row index and the model-derived tile size, workers still
receive complete fixed tasks, and results are merged in canonical order. The
unchanged PRD 0001 worker/tile matrix remains the executable determinism proof.

## Derived decisions

| shape/phase | live bytes/row | nodes | rows | node-row work | tile rows | tiled |
|---|---:|---:|---:|---:|---:|:---:|
| demographic transitions | 33 | 65 | 1,000,000 | 65,000,000 | **960** | yes |
| demographic views | 20 | 67 | 1,000,000 | 67,000,000 | 1,600 | yes |
| many-views canary | 41 | 680 | 262,144 | 178,257,920 | **768** | yes |

The demographic transition tile is within 6.25% of the known-good 1,024. The
canary deliberately puts a 41-byte-per-row balanced Real guard under each of 40
views: the old 1,024-row tile represents about 41 KiB and exceeds the 32 KiB
budget, while the derived 768-row tile remains below it.

## Two-shape measurement

Both binaries were built in release mode from the frozen baseline/workspace in
one session on an Apple M2 Pro. Each arm used five runs. Default-worker wall
time is primary; explicit single-worker measurements are reported separately.
No result is averaged across model shapes.

The demographic arm is the inherited fixed case: the exact 1M-slot synthesized
state (`896e0062…`) and resized model (`601766d8…`), four areas, present fraction
0.8, streams `birth:600,overseas:250,internal:150`, seed 9009, 24 ticks, CPU.
The canary uses 262,144 rows, 40 views, seed 9009, 24 ticks, CPU.

Fastest and median values are seconds:

| shape | workers | build | wall min / median | user min / median |
|---|---|---|---:|---:|
| demographic | default | before | 3.09 / 3.16 | 4.64 / 4.65 |
| demographic | default | after | **3.10 / 3.12** | **4.62 / 4.64** |
| demographic | 1 | before | 4.90 / 4.97 | 4.07 / 4.10 |
| demographic | 1 | after | **4.86 / 4.90** | **4.06 / 4.08** |
| many-views canary | default | before | 2.44 / 2.47 | 1.87 / 1.87 |
| many-views canary | default | after | **0.31 / 0.32** | **1.86 / 1.88** |
| many-views canary | 1 | before | 2.41 / 2.42 | 1.85 / 1.86 |
| many-views canary | 1 | after | **1.56 / 1.56** | **1.54 / 1.54** |

The demographic headline is unchanged within measurement resolution: fastest
parallel wall is 3.09 → 3.10 seconds while median wall and both user-time
figures improve. The canary improves in both parallel and single-worker arms;
default wall improves 2.44 → 0.31 seconds without increasing the fastest total
CPU time. Thus neither shape regresses, and the canary exercises both the new
work gate and a live set for which 1,024 rows exceeds budget.

Every run within and across each before/after pair produced identical primary
CSV bytes. Demographic CSV SHA-256 is
`eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
canary CSV SHA-256 is
`7e8654bae6f4619e3faee12c32a72085aeb2f22e8d1de4d7cb34b48dd7d36368`.
The full 40-run table, input identities, constants, and threshold arithmetic are
in [`measurements.json`](measurements.json).

## Acceptance gates

Final-workspace gates passed:

- `cargo test --locked`;
- `scripts/check-rust.sh`;
- `python3 scripts/check-markdown-links.py` — 137 local links in 191 tracked
  Markdown files;
- `cargo fmt --all -- --check` and `git diff --check`.

The locked suite contains PRD 0001's unchanged worker counts 1/2/4 × tile sizes
257/1,024/4,093 matrix and every checked example, CSV/hash golden, run manifest
including `final_state_sha256`, frozen state artifact, and CUDA differential
membership check. No pre-existing fixture, example, manifest, dependency, IR,
Lean file, or golden changed.
