# PRD 0002 review — Attempt 1

## Assessment

**APPROVED** — no blocking issues.

The implementation is narrowly scoped, structurally preserves the canonical racing-clock decision, and satisfies all eight acceptance criteria.

## Acceptance criteria

1. **PASS — reject-only filter:** `RacingClockFilter` computes `lo = exp(-(lambda * dt)) * (1 - margin)`. `candidate_race_time` rejects only draws below `lo`; admitted draws retain the exact platform-`ln` transform and `race_time < dt` comparison. Tests exercise rejection immediately below `lo` and admission at/above it.
2. **PASS — byte-identical goldens:** No tracked example, fixture, frozen state, golden, CLI, IR, CUDA source, or CUDA evidence path changed. All five before/after outputs have identical primary CSV, summaries, manifest, stdout, and `final_state_sha256` hashes. The full locked suite passes.
3. **PASS — unchanged firing set:** The test sweep compares guarded and oracle firing membership and firing race-time bits over 100,000 coordinates for hazards `0.001`, `0.002`, `0.0025`, `0.003`, `0.012`, `0.018`, `0.020`, `0.025`, and `1e300`. A contested 128-resource argmin comparison proves exact winning race-time bits and winners are unchanged.
4. **PASS — diagnostics:** Row-to-`entity_id` conversion occurs before filter admission. A filter that would reject every draw still produces the unchanged `EntityIdOverflow` variant, rule ID, row, and display behavior. Eager expression and contest-column preparation remain before candidate filtering in tiled and fallback paths.
5. **PASS — named margin and degenerate choice:** `RACING_CLOCK_FILTER_RELATIVE_MARGIN` is a named `1e-12` constant with the approximately 4,500-ULP structural argument. Implementation notes and evidence record the conservative choice not to special-case `1e300`: `lo == 0`, every open uniform is admitted, and exact race times remain available to any contested model.
6. **PASS — protocol and fractions:** Evidence contains five in-run measurements each side, fastest uncontended user-time headlines, medians, per-run contention flags, hardware and worker count, and per-transition enabled/`ln` counts. Ordinary transitions compute `ln` for 0.10%–2.47% of enabled candidates; the deliberately unoptimised degenerate transitions remain at 100%. The current release binary matches the recorded SHA-256.
7. **PASS — Rust gates:** Independent current-workspace runs of `cargo test --locked` and `scripts/check-rust.sh` pass.
8. **PASS — Markdown links:** `python3 scripts/check-markdown-links.py` passes with 119 local links in 169 tracked Markdown files.

## Structural and scope review

- Direct `Expr::Real` and `Expr::Param` hazards are the deliberately narrow row-invariant fragment. Row-dependent hazards retain the unfiltered oracle path.
- Prepared tiled transitions cache one filter shared by all fixed tasks; column fallback computes one filter after eager evaluation. Both use the same candidate helper.
- `rng::exp_f64_from_uniform` is exactly `-uniform.ln() / lambda`; `exp_f64` retains the same uniform construction and transform, with bit-identity coverage.
- RNG algorithm, rounds, coordinate packing, draw index, dependencies, parallel structure, IR, CUDA, and CLI implementation are unchanged.
- The platform `ln` and filter-only platform `exp` are exact documented transcendental-audit exemptions.
- Excluding managed `.piprd/**` bookkeeping, all modified and untracked paths are allowed by the PRD. No files are staged.

## Evidence checked

- Final binary SHA-256: `ef2e41667dee917a1fe2164b160bb6b455cb53c05ba86f6c0c4f96c1b6e85350`.
- Fastest uncontended user time: 6.66 s before, 5.95 s after (1.119x).
- Output identity includes manifest SHA-256 `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f` and final-state SHA-256 `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.
- Evidence JSON, formatting, and `git diff --check` pass.
