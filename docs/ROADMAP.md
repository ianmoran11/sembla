# Sembla Roadmap

**Status:** Draft, 2026-07-15. Amended 2026-07-18: ADR 0001 closed (CUDA native
`f64`); v0.4 calibration resolved to amortized NPE via an external workflow
(DECISIONS.md §G5); synthetic population generation descoped from v0.5.
Planning horizon v0.2 → v1.0.
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

**The one number that mattered, and its resolution:** the v0.1 spike measured
~74 ticks/sec at 26M rows — but in **`f32`**, because portable WGSL on Metal
exposes no shader `f64`, leaving production-`f64` throughput unanswered. The
precision spike answered it on 2026-07-18: **CUDA native `f64` on a full-rate
H100 measured ~1,380 ticks/sec at 26M rows with zero reduction error**
([ADR 0001](decisions/0001-gpu-precision.md), three verified runs). The
central technical risk is retired; v0.2 now builds the selected backend.

---

## Roadmap at a glance

| Version | Theme | Retires / unlocks | Headline decision |
|---|---|---|---|
| **v0.2** | Real GPU backend | Throughput thesis confirmed on H100; the run contract recorded (manifest) | [CUDA native `f64` selected](decisions/0001-gpu-precision.md) |
| **v0.3** | Expressiveness I: composition & observation | Views/summaries (starts now — NPE's `x`); flat n-box; the last hard-coded model leaves the CLI | Resolved 2026-07-18: flat n-box first; birth/death & ODE/Kurtz on demand |
| **v0.4** | Inference & behavior widgets | Amortized-NPE calibration; interactive widgets (trained-flow and live paths) | Resolved 2026-07-18: amortized NPE via external `sbi` (DECISIONS.md §G5) |
| **v0.5** | Policy-domain realism | Courts/queueing; the exact/tau-leap hybrid | Non-exponential durations: scheduled clocks vs. phase-type first |
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
- Precision and determinism follow [ADR 0001](decisions/0001-gpu-precision.md):
  CUDA native `f64` on verified full-rate NVIDIA hardware is selected, with
  fixed-order kernels targeting Level A on the same pinned binary and GPU model.
  Level B remains unproven.
- Backend-selection plumbing in the CLI and per-box scheduler dispatch.
- **The run manifest** (DESIGN.md §5.4) — a sidecar artifact recording the run
  contract: IR hash (beside its algorithm ID), seed, resolved θ, `dt`, ticks,
  determinism level, executing backend + precision representation + fallback
  status, enabled flags, final state hash, output hash, component versions.
  **Land this first, before the GPU backend.** It is small while one backend
  exists; the moment two do, it is a retrofit across both. It is also a
  precondition for the differential harness above: with ADR 0001 leaving the
  production precision open, a result file that cannot name the precision that
  produced it cannot participate in the gate. Today's CLI prints
  `results_sha256` / `final_state_sha256` to stdout, where they evaporate.

**✓ Decision complete — CUDA native `f64`.**
[ADR 0001](decisions/0001-gpu-precision.md) and the
[tracked H100 evidence](../spikes/precision/evidence/hyperstack-h100-20260718/README.md)
record three verified runs at 26M rows on one full-rate NVIDIA H100 PCIe. CUDA
native `f64` passed every guard with zero reduction error, winner/fired
mismatches, and unexplained mirror differences. Its median was `0.724384010`
ms/tick (35.893 billion rows/sec), retaining 112.915% of same-machine `f32`
throughput.

Double-single did not qualify on NVIDIA/Vulkan: its guard and strict-arithmetic
requirements failed, and its full row had one fired mismatch. Native `f64` via
wgpu was unavailable after an observed NVIDIA NVVM compiler failure, so CUDA is
the named production backend. The CPU `f64` oracle remains authoritative;
fixed-order CUDA targets Level A for the same pinned binary/GPU model, while
Level B remains unproven.

**Exit criteria.**
1. Every checked-in example runs GPU + CPU and passes the selected equality or
   tolerance contract; state hashes match for levels that promise bitwise
   equality.
2. The completed full-rate gate in
   [ADR 0001](decisions/0001-gpu-precision.md) remains reproducible from its
   tracked three-run evidence: CUDA native `f64` measured about 1,380.5 ticks/sec
   at 26M rows on the qualifying H100.
3. ADR 0001 and DESIGN.md §5.2 continue to state CUDA native `f64`, the unchanged
   `f64` contract, full-rate hardware restriction, and Level A consequence.
4. Every run emits a manifest; a CI test asserts that a run reproduced from
   *only* its manifest (IR hash, seed, θ, `dt`, level) matches the recorded
   output and state hashes. The contract is tested, not asserted.

---

## v0.3 — Expressiveness I: composition and observation

**Re-cut 2026-07-18**, following the NPE decision (DECISIONS.md §G5) and the
population descope. Declared views/summaries are **promoted** to start
immediately — they define the conditioning data `x` for NPE and are CPU-side
work that can run in parallel with v0.2's CUDA build. Birth/death and
ODE/Kurtz are **demoted to on-demand**: with population realism descoped, no
near-term model needs them, so they wait for the first model that does. Their
designs remain specified and binding as written.

**Goal.** Grow the model class from "static population, two boxes" to what the
NPE-era examples need: models that report through declared observation, and
more than two boxes when a model wants them — without breaking the order-free
semantics.

**Scope.**
- **Declared views and summaries** (DESIGN.md §4.6; promoted, starts now) —
  retire the hard-coded SIR branch in the runner, and provide the `x` that
  v0.4's NPE pipeline conditions on. Acceptance is deletion:
  `optional_sir_box_name` and the "sweep requires a SIR box" refusal both go,
  and the existing SIR examples keep byte-identical output through declared
  summaries. Scope is views + summaries only — no event streams, no paging.
- **Flat n-box composition** — beyond the v0.1 two-box special case: any number
  of boxes wired in one flat port graph. The operad-nesting story ("a composed
  system is a box") remains the semantics; the nesting *surface* and UI wait
  for demand.

**Deferred to demand** (specified, not scheduled):
- **Birth/death** as deterministic stream compaction — entity IDs allocated as
  `(tick, parent, slot)` (DESIGN.md §4.2); still the biggest semantic addition
  since v0.1 and the main correctness risk whenever it lands. It remains the
  first construct to land behind a default-off flag, validating the flag policy
  (DESIGN.md §5.5): flags are runtime options (never Cargo features), every
  enabled flag is recorded in the run manifest, no accepted syntax is ever
  inert. The **conflict-scope declaration syntax** decision (DESIGN.md §10.1)
  travels with it. Acceptance when it lands: deterministic and reproducible
  runs with entity-ID allocation stable under CRN; rejected with a diagnostic
  naming its flag when off; fully validated and executed when on, with the
  flag recorded in the manifest.
- **ODE/macro blocks + Kurtz** — sub-stepping internally (RK4), exposing
  sampled values per tick; the entry point for the Kurtz mean-field
  coarse-graining. Acceptance when it lands: a compartmental SIR reproduces
  the agent SIR's mean-field trajectory within the Kurtz-limit tolerance —
  demonstrated, not asserted.

**✓ Decision complete — wiring language depth: flat n-box first** (2026-07-18).
Full operad nesting with a composition UI is a large frontend + runtime
investment; the NPE-era examples don't need it. Nesting follows demand.

**Exit criteria.**
1. No model name appears in `sembla-cli`. Every example reports through declared
   views/summaries, and a test asserts that adding, removing, or disabling an
   observation leaves the run's state hash bitwise unchanged (DESIGN.md §4.6).
2. An ≥3-box wired model runs and is boundary-invariant under a hand-merge.

---

## v0.4 — Inference & behavior widgets

**Goal.** Turn simulation into decision tooling: calibrate parameters against
data, and close the interactive loop (slider → simulate → prior/posterior-
predictive plot) that the structure/behavior widget taxonomy (DESIGN.md §3)
was built to eventually support.

**Scope.**
- **Amortized NPE calibration** (DECISIONS.md §G5) on top of the existing sweep
  runner: a sweep mode accepting externally supplied θ draws (also the hook for
  any future sequential method), per-draw replica indices in the seed
  coordinate so training pairs carry independent noise (CRN stays the default
  for policy contrasts), and a thin, versioned `(θ, x)` export beside the run
  manifest feeding an external Python reference pipeline (the `sbi` stack).
- **Behavior widgets** — two latency paths: posterior-conditioned widgets query
  the trained NPE flow (milliseconds, no simulation), and live prior-predictive
  bands use the v0.2 GPU backend with scenario caching / surrogates as needed.
- Standardized summary-statistic machinery — the conditioning data `x` is
  v0.3's declared summaries (DESIGN.md §4.6), not a second, parallel notion of
  "what a run reports"; embedding networks over per-tick views are the later,
  IR-neutral extension.
- **Coordinate-derived experiment seeds** (DESIGN.md §5.3). v0.4 is where named
  axes (grids, scenario sets, calibration matrices) first appear, and therefore
  where the rule binds: a run's seed derives from a hash of its canonical
  semantic coordinate — sorted, normalized, declaration-order-independent —
  never from its positional index in the matrix. This is Sembla's own axiom
  ("randomness is a pure function of coordinates", §4.2) applied one level up.
  The failure it prevents is severe and silent: with index-derived seeds,
  inserting one case re-seeds every case after it, invalidating a matrix without
  any error. Corollaries to hold: permuting a spec's axis declarations produces
  a byte-identical experiment, and a resumed experiment is byte-identical to an
  uninterrupted one — so timestamps, attempt counts, and output paths must stay
  out of run identity.

  v0.1's prior-predictive sweep is *not* in violation and should not be rewritten
  defensively: for K i.i.d. prior draws the index is the coordinate. The rule
  applies when the sweep gains named axes — which is precisely this milestone.

**✓ Decision complete — calibration method: amortized NPE** (2026-07-18,
DECISIONS.md §G5). Simulation-based inference with a neural density estimator
trained on prior-predictive `(θ, x)` pairs; amortized rather than sequential,
so the trainer never feeds proposals back into the runner. Gradient-based
calibration stays a deferred research option (DESIGN.md §3); the
differentiable fragment gets no vote in the IR.

**✓ Decision complete — the posterior workflow lives outside the framework**
(2026-07-18, DECISIONS.md §G5). An external Python pipeline (`sbi`) consumes
the thin, versioned `(θ, x)` export; Sembla stays semantics + runtime. This
re-opens standing-no #5 explicitly and narrowly (see below). The
behavior-widget latency budget (DESIGN.md §10.6) gains an amortized path: a
trained flow evaluates in milliseconds without re-simulation.

**Exit criteria.**
1. A published example recovers known parameters of a synthetic SIR from
   simulated data via amortized NPE, reproducibly, and passes a
   simulation-based-calibration (SBC) rank check.
2. A behavior widget renders a posterior- or prior-predictive band within an
   interactive latency budget stated up front, naming which path (trained flow
   or live simulation) served it.
3. Permuting the axis declarations of an experiment spec yields byte-identical
   results, and inserting a new case leaves every existing case's seed and
   outputs unchanged (DESIGN.md §5.3).

---

## v0.5 — Policy-domain realism

**Goal.** Make the driving use case — public-policy microsimulation — real,
including the hard part v0.1 deferred: non-exponential service dynamics.
Population *generation* is no longer in scope (see the resolved decision
below); the milestone consumes populations, it does not manufacture them.

**Scope.**
- **Courts / queueing extensions** (DESIGN.md §6): scheduled clocks with
  guard-recheck at firing, top-k capacity resources, queue disciplines as
  ordering keys, matured saturation diagnostics. This is where the heterogeneous
  hybrid the operad exists for gets exercised — a large court box run *exactly*
  (sequential DES) wired to a tau-leaped population box.

**⚠ Decision point — non-exponential durations: scheduled clocks vs. phase-type
first** (DESIGN.md §4.3). Both are in the design. Scheduled clocks are "easier
on the runtime (no staleness), weaker theory at the edges"; phase-type stays
purely CTMC. Decide the order of implementation.

**✓ Decision complete — population initialization is external** (2026-07-18,
DESIGN.md §10.5). Synthetic-population generation (census/HILDA-style
microdata, reweighting, validation, privacy) is descoped from Sembla entirely:
it is an external data pipeline's product, consumed through the versioned
population format the runtime already reads. The format contract is the whole
interface. This keeps the framework's identity — semantics + runtime — focused,
and removes the roadmap's historically largest effort sink from its critical
path.

**Exit criteria.**
1. A courts model with non-exponential service times runs exactly and validates
   against the tau-leaped approximation.
2. An externally supplied population (via the versioned format) drives an
   end-to-end policy comparison using common random numbers.

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

## PFCLBS as an implementation reference (and the standing "no" list)

The sibling PFCLBS/SKS repository solves many of the same problems roughly 20×
larger (~225k lines of Rust across 12 crates, against Sembla's ~11k across 3).
It is a **reference to read at the milestone that needs it**, never a backlog to
work through. [`sembla-vs-pfclbs.md`](sembla-vs-pfclbs.md) is the full
comparison; a 2026-07-17 conservative review adopted four things (DESIGN.md
§4.6, §5.4, §5.5, and the §5.3 seed rule) on one test: *does this make a
commitment Sembla already made checkable, or does it add a new one?*

Worth reading when the milestone arrives — and not before:

| Milestone | Read | For |
|---|---|---|
| v0.2 | `crates/sks_replay` (`ReplayManifest`) | Manifest field discipline: algorithm IDs beside hashes, per-concern schema versions, append-only all-present-or-all-absent tuples |
| v0.3 | `docs/milestone-implementation-plans/feature-flag-convention.md` | Why flags are runtime options, not Cargo features |
| v0.3 | `docs/milestone-implementation-plans/er12b-lifecycle-design.md` | Bounded lifecycle: fixed capacity, generation identity, explicit overflow policy — a cross-check on DESIGN.md §4.2's `(tick, parent, slot)`, not a substitute |
| v0.3 | `docs/milestone-implementation-plans/er13b-observability-design.md` | The non-feedback invariant, stated precisely |
| v0.4 | `docs/milestone-implementation-plans/er13c-experiment-design.md` | Canonical coordinates, seed derivation, byte-identical resume |

**The standing "no" list.** These were considered and declined; a PRD proposing
one is re-opening a decision, and should say so explicitly rather than arrive as
scope:

1. **A custom surface parser.** The actual-Lean frontend is the differentiator
   (DESIGN.md §3, §8). PFCLBS pays for its own parser, spans, name resolution,
   diagnostics, and formatter; that bill is the thing Sembla declined to owe.
2. **A UI backend / browser UI.** Sembla's interface story is the Lean infoview
   (DESIGN.md §3). A second frontend would fork it.
3. **An execution-profile matrix** (DESIGN.md §8). Two paths — oracle and GPU —
   held together by differential testing, not five held together by manifests.
4. **Units/refinements in the Rust validator** (DESIGN.md §8). The goal is
   right; the location is wrong. Units belong in Lean.
5. **Calibration export / posterior import formats.** *Narrowly re-opened
   2026-07-18* (DECISIONS.md §G5): the method is now decided (amortized NPE),
   which was this entry's stated condition. The adopted surface is exactly one
   thin, versioned `(θ, x)` export beside the run manifest. Anything beyond
   that single artifact — posterior import, inference-run management, format
   families — remains declined.
6. **Replay archives, event streams, provenance databases.** The manifest
   (§5.4) is one file and stops there. Run management is a product Sembla is
   not building.
7. **PRD/evidence process machinery.** Scaled to a repository ~20× larger.

One anti-pattern worth naming, since it is the cost of the thing being admired:
PFCLBS's breadth makes it genuinely hard to tell which features are stable,
which are flag-gated, and which are aspirational — its own comparison notes it
has no top-level README, with authority scattered across `archive/`, milestone
plans, PRDs, and evidence directories. DESIGN.md-as-authority is the cheaper
discipline. Keep it.

---

## Dependency summary

```
v0.1 (done) ──> v0.2 GPU backend ──┬──> v0.4 inference + behavior widgets
                (CUDA f64 selected)│      (needs GPU latency + summaries)
                                   │
                v0.3 expressiveness┴──> v0.5 policy domain ──> v1.0 consolidation
                (views/summaries now,    (courts/queueing)     (Level B, proofs)
                 flat n-box; birth/death
                 & ODE/Kurtz on demand)

proof track ......................... parallel throughout .........................
```

- **v0.2 is the linchpin**: it retires the throughput risk and unblocks behavior
  widgets. Its precision decision cascades into Level A/B feasibility (v1.0).
- **v0.3 and v0.2 are largely independent** and could proceed in parallel with
  enough hands — v0.3 extends the CPU-oracle semantics; v0.2 makes it fast.
- **v0.4 depends on v0.2** (interactive latency) but only on the *shipped* sweep
  runner for inference, so NPE training-data generation could start early —
  and on declared summaries (§4.6) for its conditioning data `x`, which is why
  the v0.3 re-cut starts views/summaries immediately.
- **v0.5 shrank on 2026-07-18** — synthetic population, historically the
  effort sink, is descoped to an external pipeline; what remains is
  courts/queueing and the exact/tau-leap hybrid.
