---
max_review_cycles: 3
---

# PRD 0006: GPU precision decision report + DESIGN.md/roadmap amendment

## Context

This is the spike's reason for existing. PRDs 0001–0005 produce measurements;
this PRD turns them into the **decision the v0.2 backend needs**: which precision
strategy (A native `f64`, B double-single, C tiered-by-contract) Sembla adopts,
what determinism levels (`DESIGN.md` §5.2) are consequently reachable on GPU, and
what that means for the production numeric contract. The measurements are only
useful if this judgment is written down and wired back into the design authority.

Implements: `docs/ROADMAP.md` v0.2 precision fork and its exit criterion "the
precision decision is written down as a semantics amendment to DESIGN.md §5.2";
`DESIGN.md` §5.2, §9 (numeric contract), §10.3 (Level B feasibility).

## Goal

A decision record `docs/decisions/0001-gpu-precision.md` that states the chosen
strategy with its evidence, and the corresponding amendments to `DESIGN.md` and
`docs/ROADMAP.md` so the design authority reflects the resolved fork.

## Specification

- **Decision record** `docs/decisions/0001-gpu-precision.md` (create
  `docs/decisions/` if absent; ADR-style):
  - **Context:** the `f32`-only gap from the v0.1 spike and why it gates v0.2.
  - **Evidence:** the PRD-0005 matrix, summarized — throughput and accuracy for
    each strategy, and the fp64 throughput class of the NVIDIA hardware measured.
    Cite `spikes/precision/RESULTS.md` and the machines used. If the native-`f64`
    NVIDIA run was only `commodity` (rate-limited) or was not run, say so and mark
    the corresponding conclusion **provisional** rather than overclaiming.
  - **Options** A / B / C restated with the concrete numbers attached, each with
    its determinism-level consequence: e.g. native `f64` → Level A/B plausible but
    hardware-restricted and fp64-rate-dependent; double-single → portable, ~48-bit,
    Level A two-pass feasible, cost X; tiered → CPU `f64` oracle as truth, GPU
    fast path defined to permit reduced precision where the winner-mismatch rate
    is provably ~0.
  - **Decision:** the recommended strategy and the explicit trade accepted, phrased
    so it is falsifiable against the numbers. If the evidence is not decisive
    (e.g. full-rate `f64` was never measured), the "decision" is instead a
    **precise next measurement** (run PRD-0004 `full_rate`) with the threshold
    that would settle it — never a hedge dressed as a conclusion.
  - **Consequences:** what v0.2 must build, which determinism levels are in/out on
    GPU, and any change to the production `f64` convention.
- **`DESIGN.md` amendment:** update §5.2 (and §9 numeric conventions if the
  contract changes) to record the resolved precision strategy and its determinism
  consequences, citing the decision record. Keep DESIGN.md the authority — the
  decision record explains, DESIGN.md states.
- **`docs/ROADMAP.md` amendment:** mark the v0.2 "GPU precision strategy" decision
  point resolved (or "measured, pending full-rate confirmation"), linking the
  decision record; adjust the v0.2 scope/exit-criteria wording to match.
- **Honesty guardrails (inherited from the v1 spike ethos, `DESIGN.md` §9):** no
  rate-limited number extrapolated as full-rate; no `f32` result presented as
  satisfying the `f64` contract; every "unanswered" cell from RESULTS.md that
  bears on the decision is acknowledged, not silently dropped.

## Non-goals

Building the v0.2 GPU backend, implementing the chosen strategy in
`sembla-runtime`, further kernel work, new measurements beyond pointing to the
next one needed, changing the IR.

## Acceptance criteria

1. `docs/decisions/0001-gpu-precision.md` exists in ADR form with context,
   evidence (citing `spikes/precision/RESULTS.md`), options A/B/C with numbers,
   a decision (or a precise next-measurement with its deciding threshold), and
   consequences.
2. `DESIGN.md` §5.2 (and §9 if the contract changes) is amended to reflect the
   resolved strategy and cites the decision record; the change is minimal and
   keeps DESIGN.md authoritative.
3. `docs/ROADMAP.md` marks the v0.2 precision decision point resolved-or-pending
   and links the decision record.
4. The report contains no rate-limited-as-full-rate extrapolation and no
   `f32`-as-`f64` claim; provisional conclusions are labelled provisional.
5. A reader who has not run the spike can, from the decision record alone, state
   what v0.2 should build and why.
