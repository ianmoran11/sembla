# PRD 0001 revision implementation notes — Attempt 1

## Review blocker addressed

The prepared tiled evaluator previously trusted the `ParamValue` variant in `ParamEnv`, while the canonical column evaluator checked it against the target model's declared `ParamType`. This allowed a cross-model Int environment to be promoted inside an expression inferred as Real instead of producing the canonical wrong-type diagnostic.

`crates/sembla-runtime/src/eval.rs` now provides one private `checked_parameter_value` helper used by both prepared and column evaluation. It preserves declaration-before-environment lookup, the existing missing-value behavior, and the exact diagnostic:

```text
parameter environment value for '<name>' has the wrong type
```

Validation remains expression-local, so unused parameters do not start producing eager errors and existing error timing is unchanged.

## Regression coverage

`crates/sembla-runtime/src/executor.rs` adds a target model with a Real parameter used in mixed Real arithmetic and supplies a `ParamEnv` from an otherwise valid model declaring the same parameter as Int. The test captures the forced column-fallback error, then asserts the exact same error across workers 1/2/4 and tile sizes 257/1,024/4,093. The mixed expression reproduces the former silent Int-to-Real promotion rather than merely triggering a later root-type mismatch.

## Evidence refresh

The revised release binary SHA-256 is:

```text
5b93233b42429da08021eb8eb2945465505f5ca13ee0be680437da6616b86fb8
```

All after-binary measurements were rerun and `docs/evidence/evaluator-tiled-tick-20260727/{README.md,measurements.json}` were refreshed. The unchanged baseline five-run corpus was retained. Revised fastest uncontended figures are:

- before: 7.71 s wall / 6.00 s user;
- tiled, 1 worker: 7.32 s wall / 6.10 s user;
- tiled, 10 workers: 5.47 s wall / 6.66 s user.

All revised primary CSV, summaries, manifests, stdout, and `final_state_sha256` values remain byte-identical to the baseline. Two non-headline 262,144-row single-worker threshold runs were flagged contended in the machine-readable evidence; the reported fastest run was uncontended.

## Checks

Passed in the final workspace:

- focused wrong-typed-parameter regression;
- tiled worker/tile determinism tests;
- sequential Real aggregate fallback test;
- `cargo test --locked`;
- `scripts/check-rust.sh`;
- `python3 scripts/check-markdown-links.py` (118 links in 168 tracked Markdown files);
- JSON validation for `measurements.json`;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- allowed-path and tracked golden/CUDA evidence path checks.

A fresh read-only reviewer found no new blocker and confirmed the shared resolver, error ordering, 3×3 regression, revised-binary hash, and evidence consistency.

No commit was made, as required.
