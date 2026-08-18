# PRD track index

Implementation specifications are retained at their established paths because the PRD runner, reviews, evidence, and historical links refer to them. They are records, not maintained user guidance.

The executable pending-work source is [`../prds-run-queue/README.md`](../prds-run-queue/README.md). A track README records its own status and acceptance evidence.

Tracks are retained at their established paths rather than moved when they close,
because `DECISIONS.md`, archived roadmaps, and retained evidence READMEs link to
them and those records are immutable. Standing is therefore marked here.

**Standing:** `active` — on the critical path to the current milestone.
`background` — retained and usable, but not currently prioritised.
`closed` — its work landed. `superseded` — a later decision replaced its approach.

| Track | Standing | Purpose |
| --- | --- | --- |
| [`prds/`](README.md) | closed | original V0.1 implementation sequence |
| [`prds-run-queue/`](../prds-run-queue/README.md) | active | ordered queue for pending PRDs |
| [`prds-demographic-spine/`](../prds-demographic-spine/README.md) | active | aggregate-first demographic calibration and Lean-only person-facet foundation |
| [`prds-lean-ir-formalization/`](../prds-lean-ir-formalization/README.md) | active | proof-complete Lean frontend and V1 IR semantics |
| [`prds-demographic-slots/`](../prds-demographic-slots/README.md) | background | demographic fixed-slot model, state artifacts, and grouped observations |
| [`prds-australian-population/`](../prds-australian-population/README.md) | background | ABS-calibrated Australian microsimulation; follows the synthetic pipeline proof |
| [`prds-composition/`](../prds-composition/README.md) | closed | canonical composition-source and linker architecture |
| [`prds-composition-integration/`](../prds-composition-integration/README.md) | closed | plan execution, comparison, CUDA, and widgets |
| [`prds-surface-syntax/`](../prds-surface-syntax/README.md) | closed | mathematical Lean surface language |
| [`prds-proof-track/`](../prds-proof-track/README.md) | closed | initial Lean proof work |
| [`prds-indexed-families/`](../prds-indexed-families/README.md) | closed | indexed family construction |
| [`prds-contract-governance/`](../prds-contract-governance/README.md) | active | versioned contract and parameter governance |
| [`prds-project-hygiene/`](../prds-project-hygiene/README.md) | background | repository checks, CI, policies, and cleanup |
| [`prds-npe-path/`](../prds-npe-path/README.md) | superseded | neural posterior estimation workflow; `DECISIONS.md` §O1 estimates at the cheapest identified level instead. Its retained 2026-08-07 evidence stays immutable under §O2 |
| [`prds-precision-spike/`](../prds-precision-spike/README.md) | closed | GPU precision decision evidence |
| [`prds-portable-sampler/`](../prds-portable-sampler/README.md) | background | portable sampling behavior |
| [`prds-cuda-validation-parallelism/`](../prds-cuda-validation-parallelism/README.md) | background | validation and claim-resolution CUDA parallelism |
| [`prds-cuda-host-path/`](../prds-cuda-host-path/README.md) | background | CUDA host-state and control-transfer reductions |
| [`prds-cuda-final-state-readback/`](../prds-cuda-final-state-readback/README.md) | background | final-state readback and hashing |
| [`prds-cuda-transition-families/`](../prds-cuda-transition-families/README.md) | background | profile-gated CUDA per-transition launch investigation |
| [`prds-contract-governance-gpu/`](../prds-contract-governance-gpu/README.md) | background | GPU-gated contract governance run |
| [`prds-device-observation/`](../prds-device-observation/README.md) | background | device-side observation paths |
| [`prds-evaluator-throughput/`](../prds-evaluator-throughput/README.md) | background | CPU evaluator parallelism and tiling |
| [`prds-host-evaluator-performance/`](../prds-host-evaluator-performance/README.md) | background | host evaluator profiling and optimization |
| [`prds-execution-timing/`](../prds-execution-timing/README.md) | background | phase-level execution timing |
| [`prds-sweep-throughput/`](../prds-sweep-throughput/README.md) | background | sweep execution and concurrent CUDA draws |

Every CUDA and throughput track is `background` for one reason: CPU is the
authoritative backend for the current milestone, so accelerator throughput is
not on the critical path. Their evidence stands and none of it was retracted.

## Record policy

- Move a pending PRD into the run queue only according to its queue contract; do not copy it.
- Return approved PRDs to their binding track.
- Do not rewrite completed PRDs to describe current behavior. Amend the track README or maintained documentation instead.
- Keep `.piprd/` review and implementation state separate from reader documentation.
- Link current user guidance through [`../README.md`](../README.md), not through a PRD.
