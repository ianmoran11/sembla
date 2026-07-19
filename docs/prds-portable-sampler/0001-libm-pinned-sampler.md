# PRD 0001: `libm`-pinned sampler transcendentals

## Context

See the folder README for the CI finding and decision. The sampler
(`crates/sembla-runtime/src/prior.rs`) computes Normal via Box–Muller —
`(-2.0 * u0.ln()).sqrt() * (2.0 * PI * u1).cos()` — and LogNormal via
`normal.exp()`. The `ln`/`cos`/`exp` calls resolve to the platform libm, so
draw bits differ across platforms by ULPs. The pure-Rust `libm` crate
(musl-derived algorithms, compiled under Rust's strict FP semantics — no
fast-math, no FMA contraction of written expressions) produces identical bits
on every platform for a pinned crate version.

## Goal

Parameter draws are a pure function of coordinates **and nothing else** — same
bits on macOS/aarch64 and Linux/x86_64 — with the dependency, frozen vectors,
and decision record all updated coherently.

## Specification

- Add `libm` (exact pinned version, `=x.y.z`) to `sembla-runtime`'s
  dependencies; extend the `scripts/check.sh` allowlist from `sha2` to
  `sha2` + `libm`.
- In `prior.rs`, replace `f64::ln` → `libm::log`, `f64::cos` → `libm::cos`,
  `f64::exp` → `libm::exp`. Keep `sqrt` on `std` (IEEE-exact; document in the
  module docs why it is exempt). Algorithm, counters, constant, and expression
  order are unchanged — the module's existing documentation of the mapping
  stays true except for the implementation source, which it now names.
- Audit the rest of `sembla-runtime` and `sembla-cli` for other
  libm-backed `f64` methods (`ln`, `log*`, `exp*`, `powf`, `sin`, `cos`,
  `tan`, `asin`…, `sinh`…) reachable from any result-bearing path; the
  authoring-time grep found none outside `prior.rs`, but the audit is the
  PRD's to confirm. Add a guard test that greps `crates/sembla-runtime/src`
  and `crates/sembla-cli/src` for those `std` method calls and fails on any
  hit outside an explicit documented exemption list (initially: none).
- Regenerate the frozen sampler vectors in
  `crates/sembla-runtime/tests/prior.rs` from the `libm`-backed
  implementation. Every other sampler test (moments, namespace stability,
  K-independence) must pass unchanged — they are distribution-level and must
  not need new expected values; if one does, stop and flag it (it would mean
  the algorithm changed, not just the implementation).
- Record the decision in `DECISIONS.md` as **§G6** (dated 2026-07-19):
  consideration (platform-libm ULP divergence found by CI's first run; θ
  portability is load-bearing for the NPE workflow), rationale (pure-Rust
  `libm`, pinned; narrow Level-B-style pinning on the cold path; `sqrt`
  exempt; one allowlist extension), and the honest breaking-change statement:
  θ draws from sweeps generated before this commit are not bit-reproducible
  after it.
- One-sentence mentions where users will look: the sweep section of
  `docs/examples/sir.md` (θ draws are platform-independent as of this change)
  and the root `README.md` conventions if it names runtime dependencies.

## Non-goals

Pinning transcendentals in the simulation path (there are none — the audit
confirms rather than changes this). Level B for run outputs. Touching the
CUDA backend, the Box–Muller algorithm, prior families, or reserved
namespaces. Downstream fixture regeneration (PRD 0002).

## Acceptance criteria

1. `cargo test --workspace` green, including regenerated frozen vectors; the
   distribution-level sampler tests pass **without** modification.
2. `./scripts/check.sh` passes with the two-entry allowlist; a test or check
   asserts `libm` is pinned exactly (`=` requirement in `Cargo.toml`).
3. The guard test fails if a `std` transcendental method call is introduced
   into `sembla-runtime`/`sembla-cli` source (demonstrated in the PRD's tests
   by construction, e.g. compile-time fixture or a self-test of the grep).
4. `DECISIONS.md` §G6 exists with the stated content; docs mentions landed.
5. The old→new bit change is demonstrated: a test comment or fixture records
   the previously frozen LogNormal probe value beside the new one, so the
   break is visible in review rather than silent.
