# PRD 0002 implementation notes — Attempt 1

## Implementation

- Added the named `RACING_CLOCK_FILTER_RELATIVE_MARGIN = 1e-12` constant in `crates/sembla-runtime/src/executor.rs`, with the approximately 4,500-ULP protective argument.
- Limited cached filters to structurally row-invariant direct `Expr::Real` and `Expr::Param` hazards. Row-dependent hazards continue through the original unfiltered path.
- Cached one filter per prepared transition/tick for tiled execution and one per transition/tick in the column fallback after eager guard, hazard, and contest-column evaluation.
- Added one shared candidate racing-clock helper used by both paths. It performs `u32::try_from(row)` before filtering, draws at the unchanged Philox coordinates, rejects only `u < lo`, and otherwise retains the canonical transform and `race_time < dt` comparison.
- Extracted `exp_f64_from_uniform` in `crates/sembla-runtime/src/rng.rs`; its body is exactly the previous `-uniform.ln() / lambda`, and `exp_f64` still uses the same uniform construction and transform.
- Updated the transcendental audit test with exact exemptions for the unchanged §E7 `ln` and the reject-only `exp` bound.

## Degenerate-hazard choice

The implementation deliberately does not special-case the two `1e300` hazards. Their threshold and lower bound are zero, and `uniform_f64` excludes zero, so every enabled candidate is admitted and computes its exact race time. This is conservative for arbitrary future contested models and leaves the §E1 counter-coordinate draw behavior unchanged. No consumed draw or race-time value is skipped.

## Tests

Added coverage proving:

- the filter rejects immediately below `lo` and admits `lo` and above;
- guarded and oracle firing sets and firing race-time bits match for 100,000 coordinates at each hazard `0.001`, `0.002`, `0.0025`, `0.003`, `0.012`, `0.018`, `0.020`, `0.025`, and `1e300`;
- contested argmin winners over 128 resources retain exact race-time bits;
- a rejecting filter cannot hide the unchanged `EntityIdOverflow` path;
- the split uniform/transform result is bit-identical to `exp_f64`.

Existing tiled determinism and full oracle/golden tests also pass.

## Evidence

New evidence is under `docs/evidence/evaluator-guarded-racing-clock-20260727/`.

Binding protocol, fastest uncontended runs:

- before: 5.34 s wall / 6.66 s user;
- after: 5.35 s wall / 5.95 s user;
- user-time speedup: 1.119x (10.66% reduction);
- headline wall time: essentially flat at 0.998x.

All five before and five after runs were uncontended. All primary CSV, summaries, manifests, stdout, and `final_state_sha256` hashes were byte-identical. The final release binary SHA-256 is:

```text
ef2e41667dee917a1fe2164b160bb6b455cb53c05ba86f6c0c4f96c1b6e85350
```

A temporary measurement-only binary counted filter admission and was not used for timings. Instrumentation was reverted and the final binary rebuilt. The eight ordinary transitions computed `ln` for 0.10%–2.47% of enabled candidates; the deliberately unoptimised `1e300` transitions remained at 100%.

## Final checks

Passed:

- focused guarded-clock, filter, contest, overflow, and split-transform tests;
- `cargo test --locked`;
- `scripts/check-rust.sh`;
- `python3 scripts/check-markdown-links.py` (119 links in 169 tracked Markdown files);
- `cargo fmt --all -- --check`;
- Clippy with warnings denied;
- evidence JSON validation;
- release-binary hash verification;
- allowed-file scope, unchanged golden/CUDA paths, and `git diff --check`.

A fresh read-only implementation audit found no blocker. No commit was made, as required.
