# Sembla Roadmap

**Status:** Draft, 2026-07-15. Planning horizon v0.2 → v1.0.
**Authority:** [`DESIGN.md`](../DESIGN.md) is the design authority; this roadmap
sequences its §9 "Out" backlog and resolves its §10 open questions into ordered
milestones. Where this roadmap and DESIGN.md conflict, DESIGN.md wins — flag it.

This is a *direction* document, not a commitment schedule. Milestones are
themed and dependency-ordered; each names its goal, scope, exit criteria, and
the decision points that gate it. Durations are deliberately omitted — the
sequencing and the forks are the point.

---

## Where we are: v0.1 is complete

All 13 v0.1 PRDs (`docs/prds/0001`–`0013`) are implemented and the DESIGN.md §9
success criteria are met:

- **CPU reference interpreter** (`sembla-runtime`, ~4.3k LOC) — the executable
  semantics oracle, Level A determinism (bitwise, same binary/machine).
- **IR + validator** (`sembla-ir`) — versioned wire format, first-class
  parameters with declared priors, `rule_id` assignment, canonical serializer.
- **Philox-by-coordinate RNG**, columnar ACSet state with canonical SHA-256,
  allocation-free kernel expression evaluator.
- **Tau-leaped tick executor** — racing clocks, contested resources, argmin
  resolution with lexicographic tie-break, saturation diagnostic.
- **Composition** — two boxes + one feedback wire, one-tick delay, verified
  boundary-invariant against a hand-merged model.
- **Lean 4 frontend** — surface DSL → deep-embedded IR, positive/negative
  elaboration tests, two structure widgets (state diagram, prior-marginal plot).
- **CLI** — `validate`, `run`, `sweep`, `compare` (common-random-numbers),
  `diff-ir`. Prior-predictive sweep runner reproducible end-to-end from one seed.
- **GPU throughput spike** (throwaway) — measured on Apple M2 Pro / Metal.

**The one number that matters, and its asterisk:** the spike measured
~74 ticks/sec at 26M rows — but in **`f32`**, because portable WGSL on Metal
exposes no shader `f64`. Production-`f64` throughput is **unanswered**
(`spikes/gpu-throughput/RESULTS.md`). This is the central technical risk the
roadmap must retire first, and it shapes v0.2.

---

## Roadmap at a glance

| Version | Theme | Retires / unlocks | Headline decision |
|---|---|---|---|
| **v0.2** | Real GPU backend | Throughput thesis, for real | [Precision measured; pending full-rate gate](decisions/0001-gpu-precision.md) |
| **v0.3** | Expressiveness I: dynamic populations | Birth/death, general composition, ODE + Kurtz | How far to push the wiring-diagram language |
| **v0.4** | Inference & behavior widgets | Calibration; interactive slider→simulate→plot | Black-box (ABC/SBI) vs. gradient-based |
| **v0.5** | Policy-domain realism | Courts/queueing; synthetic population | Where population initialization lives |
| **v1.0** | Consolidation & guarantees | Level B (if feasible); first landed proofs | Is portable-bitwise practical or aspirational |

A **proof track** (§ below) runs in parallel throughout, opportunistically, and
is never on the critical path.

---

## v0.2 — Real GPU backend

**Goal.** Replace the throwaway spike with a production GPU backend that is
*differentially tested against the CPU oracle*: same IR + seed + θ, GPU output
matches the oracle to the guaranteed tolerance for the chosen determinism level.
This is the milestone that makes "GPU-shaped by construction" (DESIGN.md §4.2)
pay rent.

**Scope.**
- GPU execution of the closed kernel fragment: map / filter / join-on-keys /
  group-by-monoid / segmented argmin / Philox draws, resident on device across
  ticks.
- Differential test harness: the oracle is ground truth; every example model
  runs on both paths in CI and outputs are compared under the selected numeric
  contract. State hashes must match only when the declared level promises
  bitwise equality; tolerance-based paths compare values, winner/fired flags,
  and contract diagnostics instead.
- Precision and determinism follow the full-rate gate in
  [ADR 0001](decisions/0001-gpu-precision.md): Level A is the target for a
  qualifying native-`f64` or strict double-single path; Level C is permitted only
  with an explicit reduced-precision contract.
- Backend-selection plumbing in the CLI and per-box scheduler dispatch.

**⚠ Decision point — GPU precision strategy: measured, pending full-rate
confirmation.** [ADR 0001](decisions/0001-gpu-precision.md) records the evidence
and binding decision rule. On Apple M2 Pro, double-single retained 77.2% of
`f32` throughput, reduced max relative error from `1.714669e-7` to
`1.096998e-14`, and had zero observed winner mismatches. Native wgpu `f64` was
unsupported and CUDA was not run; no NVIDIA fp64 class or native timing was
measured.

Before precision-dependent backend work, run the complete matrix and the ADR's
supplemental guard diagnostics three times on one verified PRD-0004 `full_rate`
NVIDIA instance. The gate uses each preserved file's NVIDIA-local embedded rows,
not the rendered cross-machine matrix; distinct result paths are required because
a rerun replaces the fixed NVIDIA entry.

Select qualifying native `f64` when double-single does not qualify, or when both
qualify and the named native production path is at least 20% faster. Otherwise
select qualifying double-single. Tiered precision is available only when neither
precise candidate qualifies and an explicit reduced contract passes. Until then
the CPU `f64` oracle and the existing numeric contract remain authoritative, and
Level B remains unproven.

**Exit criteria.**
1. Every checked-in example runs GPU + CPU and passes the selected equality or
   tolerance contract; state hashes match for levels that promise bitwise
   equality.
2. The full-rate gate in [ADR 0001](decisions/0001-gpu-precision.md) is run at
   26M rows, and the selected `f64`-compliant or explicitly contract-defined
   path has measured ticks/sec on its qualifying hardware.
3. ADR 0001 is updated from pending to the selected strategy and DESIGN.md §5.2
   states that strategy, tolerance, and determinism consequence.

---

## v0.3 — Expressiveness I: dynamic populations

**Goal.** Grow the model class from "static population, two boxes" to the
constructs real policy models need, without breaking the order-free semantics.

**Scope.**
- **Birth/death** as deterministic stream compaction — entity IDs allocated as
  `(tick, parent, slot)` (DESIGN.md §4.2). This is the biggest semantic addition
  since v0.1 and the main correctness risk of the milestone.
- **General n-box composition** — beyond the v0.1 two-box special case: the
  wiring-diagram language, arbitrary port graphs, and the operad-nesting story
  ("a composed system is a box").
- **ODE/macro blocks** — sub-stepping internally (RK4), exposing sampled values
  per tick; the entry point for the **Kurtz mean-field** coarse-graining
  (agent-population ↔ ODE as the same object at two resolutions).

**⚠ Decision point — how far to push the wiring-diagram language.** Flat n-box
wiring is a modest step from v0.1; full operad nesting with a composition UI is
a large frontend + runtime investment (DESIGN.md §9 lists "general
wiring-diagram language and box nesting UI" as explicitly out of v0.1). Decide
whether v0.3 ships flat-n-box (defer nesting UI) or commits to the full nested
operad. Recommend flat-n-box first; nesting UI follows demand.

**⚠ Decision point — conflict-scope declaration syntax** (DESIGN.md §10.1).
Birth/death and richer models make the "a transition's claimed resources must
cover its writes" obligation load-bearing. Settle the surface syntax and the
elaboration-time coverage check here.

**Exit criteria.**
1. A model with birth and death runs deterministically and reproducibly; entity
   ID allocation is stable under CRN.
2. An ≥3-box wired model runs and is boundary-invariant under a hand-merge.
3. A compartmental SIR reproduces the agent SIR's mean-field trajectory within
   the Kurtz-limit tolerance — the coarse-graining demonstrated, not just
   asserted.

---

## v0.4 — Inference & behavior widgets

**Goal.** Turn simulation into decision tooling: calibrate parameters against
data, and close the interactive loop (slider → simulate → prior/posterior-
predictive plot) that the structure/behavior widget taxonomy (DESIGN.md §3)
was built to eventually support.

**Scope.**
- **Calibration/inference architecture** on top of the existing sweep runner —
  which already provides "everything black-box methods need" (DESIGN.md §10.4).
- **Behavior widgets** — gated on runtime latency, now unblocked by the v0.2 GPU
  backend. Scenario caching and/or surrogate models as needed for interactivity.
- Standardized summary-statistic / distance machinery for simulation-based
  inference.

**⚠ Decision point — calibration method** (DESIGN.md §10.4). The design leans
black-box: ABC / simulation-based inference drive the sweep runner and never
reach into the IR; only gradient-based calibration would require the
differentiable fragment, which DESIGN.md §3 keeps deliberately deferred. Decide:
commit v0.4 to SBI/ABC (low-risk, builds on shipped infrastructure), or open the
differentiable-fragment research track (option value, not a requirement). Strong
recommendation: **SBI/ABC for v0.4**; gradients stay a research spike.

**⚠ Decision point — where the posterior workflow lives** (DESIGN.md §10.4).
In-framework (Sembla owns the inference loop) vs. thin adapter feeding an
external probabilistic-programming stack. Also: the behavior-widget latency
budget (§10.6) — what interactive loop the GPU backend actually affords.

**Exit criteria.**
1. A published example recovers known parameters of a synthetic SIR from
   simulated data via the chosen method, reproducibly.
2. A behavior widget renders a prior-predictive band from live simulation within
   an interactive latency budget stated up front.

---

## v0.5 — Policy-domain realism

**Goal.** Make the driving use case — public-policy microsimulation on a
synthetic Australian population — real, including the two hard parts v0.1
deferred: non-exponential service dynamics and population initialization.

**Scope.**
- **Courts / queueing extensions** (DESIGN.md §6): scheduled clocks with
  guard-recheck at firing, top-k capacity resources, queue disciplines as
  ordering keys, matured saturation diagnostics. This is where the heterogeneous
  hybrid the operad exists for gets exercised — a large court box run *exactly*
  (sequential DES) wired to a tau-leaped population box.
- **Synthetic population initialization** (DESIGN.md §10.5) — census/HILDA-style
  microdata, reweighting (IPF/synthetic reconstruction), validation, privacy.
  Historically **>50% of the effort** in policy microsimulation and unaddressed
  by the architecture; it must not be underestimated.

**⚠ Decision point — non-exponential durations: scheduled clocks vs. phase-type
first** (DESIGN.md §4.3). Both are in the design. Scheduled clocks are "easier
on the runtime (no staleness), weaker theory at the edges"; phase-type stays
purely CTMC. Decide the order of implementation.

**⚠ Decision point — where population initialization lives.** Build a
first-class synthetic-population pipeline *inside* Sembla, or define it as an
external data pipeline that emits the versioned population format the runtime
already consumes. Recommend **external pipeline + strong format contract** to
keep the framework's identity (semantics + runtime) focused — but this is a
genuine scope call with big consequences for the product.

**Exit criteria.**
1. A courts model with non-exponential service times runs exactly and validates
   against the tau-leaped approximation.
2. A synthetic population is generated, validated against target marginals, and
   drives an end-to-end policy comparison using common random numbers.

---

## v1.0 — Consolidation & guarantees

**Goal.** Stabilize the wire format and CLI, and convert the "guarantees" from
design promises into shipped reality where feasible.

**Scope.**
- **Determinism Level B** (portable bitwise) — *if* the v1.0 decision below says
  it is practical. Software-pinned FP, no FMA/fast-math, fixed order everywhere.
- **First landed proofs** — promote one or more theorem *statements* (below) to
  actual Lean proofs, starting with the cheapest high-value target.
- IR/CLI stability commitments, migration guarantees, docs consolidation.

**⚠ Decision point — Level B feasibility** (DESIGN.md §10.3). How expensive
portable-bitwise FP really is on modern GPUs — practical for published results,
or aspirational? This depends directly on the v0.2 precision decision. If Level B
proves impractical, v1.0 should say so honestly and document the achievable
guarantee rather than ship a hedge.

**Exit criteria.**
1. Level B either delivered (bitwise across two different GPUs on a published
   example) or formally documented as not-in-v1 with the reason.
2. At least one theorem target proven in Lean against the IR semantics.
3. Wire format versioned with a stated compatibility policy.

---

## Proof track (parallel, opportunistic)

DESIGN.md §7 specifies the theorem targets now and defers the proofs; §3 notes
proofs are "expected to get cheaper as AI-assisted proving matures." This track
runs alongside the milestones and is never a release blocker. Suggested order,
cheapest-useful first:

1. **Group-by lumping rewrite correctness** (§7 example) — the flagship
   optimization = certified equivalence. Best first proof: concrete, high-value,
   directly motivates the Lean investment.
2. **Refactoring invariance** — re-drawing box boundaries preserves semantics
   (consequence of uniform one-tick delay). Naturally paired with v0.3's general
   composition work.
3. **Composition laws** — the operad-algebra axioms for box wiring.
4. **Kurtz mean-field limit** for the compartmentalizable fragment — paired with
   v0.3's ODE/coarse-graining work.
5. **Symbolic gradient correctness** over ℝ — only if the differentiable
   fragment materializes (v0.4 research spike outcome).

---

## Cross-cutting decision points (not tied to one milestone)

- **Δt discipline** (DESIGN.md §10.2) — guidance and diagnostics for choosing
  tick size per box, and automatic tau-leap bias detection beyond the saturation
  counter. Touches every milestone; needs an owner early.
- **Cross-boundary tick-delay ergonomics** (DESIGN.md §10.7) — the uniform
  one-tick message delay is honest but must be *taught*; a docs/tooling
  responsibility that grows with v0.3's composition work.
- **Toolchain risk** (DESIGN.md §3) — Lean widget API churn and VS Code coupling
  are accepted risks; the frontend-agnostic IR is the standing hedge. Revisit if
  widget breakage starts costing milestones.

---

## Dependency summary

```
v0.1 (done) ──> v0.2 GPU backend ──┬──> v0.4 inference + behavior widgets
                (precision fork)   │      (needs GPU latency)
                                   │
                v0.3 expressiveness┴──> v0.5 policy domain ──> v1.0 consolidation
                (birth/death, n-box,     (courts, synth pop)   (Level B, proofs)
                 ODE/Kurtz)

proof track ......................... parallel throughout .........................
```

- **v0.2 is the linchpin**: it retires the throughput risk and unblocks behavior
  widgets. Its precision decision cascades into Level A/B feasibility (v1.0).
- **v0.3 and v0.2 are largely independent** and could proceed in parallel with
  enough hands — v0.3 extends the CPU-oracle semantics; v0.2 makes it fast.
- **v0.4 depends on v0.2** (interactive latency) but only on the *shipped* sweep
  runner for inference, so its black-box path could start early.
- **v0.5 is the most under-scoped** — synthetic population is historically the
  effort sink; treat its estimate with suspicion.
