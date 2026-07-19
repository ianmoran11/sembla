# Portable-sampler PRDs

Two-PRD set making parameter draws **bitwise-portable across platforms**, run
by [pi-piprd](https://github.com/ianmoran11/pi-piprd)
(`/piprd run docs/prds-portable-sampler`). This README is excluded from runs.

## Motivation (the CI finding, 2026-07-19)

CI's first run failed `sampler_mapping_is_bitwise_frozen` on ubuntu x86_64 by
**1 ULP**: the LogNormal sampler produced `…240` on the macOS/aarch64 machine
that generated the frozen vectors and `…241` on Linux. Cause: the sampler uses
`std` `f64::ln/cos/exp`, which delegate to the *platform* libm, and libm
transcendentals differ by ULPs across platforms. IEEE basic ops (`+ − × ÷
sqrt`) are bitwise-portable; transcendentals are not.

Nothing promised was violated (Level A is same-binary/same-machine, and the
same-machine determinism CI job passed) — but θ draws that differ across
machines break the NPE workflow's practical story: training pairs generated on
a GPU box must be θ-identical to the same sweep on the development machine.
Decision (2026-07-19, operator-approved): software-pin the sampler's
transcendentals via the pure-Rust `libm` crate — Level-B-style pinned FP,
applied narrowly to the cold θ-draw path where it costs nothing.

**Scope fact, verified before authoring:** the simulation hot path
(`eval.rs`, `executor.rs`) and the CUDA codegen use **no** transcendentals —
which is why CPU/CUDA bitwise equality holds. The blast radius is
`prior.rs` plus every fixture downstream of θ draws. The CUDA backend is
untouched.

## Authority

`DESIGN.md` (§5.2 determinism levels, §5.3 seeds/CRN), `DECISIONS.md` §G4–G5,
`docs/prds/README.md` and `docs/prds-npe-path/README.md` conventions — all
binding. Where a PRD conflicts with DESIGN.md, flag it and follow DESIGN.md.

## Run order

| # | PRD | Layer |
|---|-----|-------|
| 0001 | `libm`-pinned sampler transcendentals | runtime |
| 0002 | Downstream fixture regeneration + cross-platform proof | fixtures/CI |

## Conventions binding on this set

- **Algorithms unchanged, implementations swapped.** Box–Muller stays exactly
  as documented (cosine branch, same counters, same expression order); only
  `ln`, `cos`, `exp` move from `std` to `libm::{log, cos, exp}`. `sqrt` stays
  `std` (IEEE-exact everywhere) — document that choice.
- **`libm` is pinned to an exact version** in `Cargo.toml`; a version bump can
  change draw bits and must be treated as a frozen-vector-breaking change.
- **The allowlist change is deliberate:** `scripts/check.sh` currently allows
  `sembla-runtime` exactly one external dependency (`sha2`). Extending it to
  `libm` is a recorded decision (PRD 0001 adds the DECISIONS.md entry), not a
  drive-by edit.
- **Breaking-change honesty:** previously generated sweeps' θ draws are not
  bit-reproducible after this change. That is the point of doing it now —
  before the `(θ, x)` export calcifies. The DECISIONS entry says so plainly;
  no compatibility shim is built.
- **Honest reporting:** the cross-platform proof ultimately comes from CI on
  Linux. A criterion that cannot be verified on the implementing machine is
  reported *unanswered* with the exact command/observation that will answer
  it — never assumed.
