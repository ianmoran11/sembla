# Sembla forward roadmap: two driver models, one implementation path

Status: **adopted 2026-07-25** — committed to the repository, which §0 defines as the adoption act; `docs/ROADMAP.md` carries the corresponding amendment note. Forward-looking planning document. Drafted 2026-07-23; revised 2026-07-24 (review pass: adoption/authority rules added as §0; missing evidence-producer, validation, and trace/explain PRDs added; Track J restructured with per-contract gates; the gate register made normative; earlier review comments resolved in place where the repository already dictates an answer). Second review pass 2026-07-25, conducted against the repository as committed at `3c0ca90` and its measured evidence; the comments it adds are open and carry no resolution. Companion to `scientific-limitations.md`, which records what the current demographic fixture is and is not scientifically. This document records where Sembla—the simulation software—should go next, the two use cases intended to steer it, and a staged implementation roadmap organised by PRD folder.

It synthesises the comment threads in `scientific-limitations.md`, in particular the question: *what should change about Sembla as a whole?*

> [!note] Second review pass (2026-07-25) — what changed
> The two-driver thesis, the serial rule, the gate discipline, and the priority order's substance all survived review unchanged. Seventeen comments were raised and resolved in place; the revisions they produced are:
>
> **New normative rules.** A *construct-completeness rule* (§5) requiring every semantics-adding PRD to declare its backend story and its surface story — grouped observations landed CPU-only and this roadmap then depended on it at national scale, and Track B's claim contract was queued to repeat the pattern. A *queue-ordering rule* (§5) ordering the runner queue by blocking resource rather than by track, since runner throughput exceeds data-acquisition throughput by orders of magnitude. An *artifact-retention contract* (Stage 1) capping chained-window storage, which reaches terabytes at national scale.
>
> **New scheduled work.** A Preflight *scale-evidence measurement* (§0) that decides the geography bound, the §K9 CUDA-grouped deferral, and the C.0002 corpus scope — three quantitative commitments previously resting on extrapolation from one laptop measurement. A *baseline-adequacy gate* (§6) closing Stage 1, whose failure pattern is the cheapest evidence this plan can buy. *C.0008*, resolving the legacy/plan execution fork before C.0005 writes a compatibility policy over it.
>
> **Reordering.** J.0001 moves to Preflight, so the second driver is on the table before the contracts it arbitrates freeze. C.0007 moves to Preflight, resolving a stated priority that had been scheduled thirtieth. Stage 1 is reordered by blocking resource. A minimum spine is marked in the normative order.
>
> **Content the tracks assumed but did not own.** The annual-to-monthly rate conversion and small-area estimation method (E.0002); census-perturbation reconciliation (E.0005), a precondition for M.0003's exact-balance claim rather than a refinement of it; classification edition and concordance policy (E.0001); justice data availability and a restated observation-capability claim (J.0001); Monte-Carlo replication design and a committed location for gate predeclarations (§6); a machine-readable limitations register (§7).

> [!warning] Review comment — Authority and adoption
> This file does not say whether it supersedes or amends the repository's `docs/ROADMAP.md`, which in turn names `DESIGN.md` as the design authority. Add an adoption date/status, map these stages to the existing milestones, and state how conflicts with `DESIGN.md` and `DECISIONS.md` are resolved; an unresolved conflict should keep the affected PRD from becoming ready.
>
> **Resolution (2026-07-24):** addressed by §0 below.

## 0. Authority, adoption, and in-flight work

- **Adoption.** *Discharged 2026-07-25:* this document is committed as `docs/forward-roadmap.md` with a dated amendment note in `docs/ROADMAP.md`. It is now a roadmap of record alongside `docs/ROADMAP.md`, which retains the version milestones; where they conflict, `DESIGN.md` and `DECISIONS.md` win over both, and the conflict is resolved by explicit amendment rather than by precedence between the two roadmaps.
- **Authority.** `DESIGN.md` remains the design authority and `DECISIONS.md` the decision record; this roadmap sequences work and never overrides either. Where a stage below requires revisiting an adopted decision (Track D vs §K5/§K9; Track B vs §K9; Track J vs §C6), the amendment is scheduled as explicit work, and the affected PRD is not `Ready` until the amendment is accepted in `DECISIONS.md` or the PRD is closed.
- **Milestone mapping.** Stages 1–2 build the empirical baseline that `docs/ROADMAP.md` v0.5 presupposes ("the milestone consumes populations, it does not manufacture them"); Track J *is* the v0.5 courts/queueing theme, executed under an evidence gate; C.0005/C.0006 are the v1.0 consolidation items; C.0002 continues the v0.2 differential-testing obligation. The Lean proof track remains as `docs/ROADMAP.md` defines it — parallel, opportunistic, never on the critical path — and is exempt from the serial PRD rule below because it is never `Active` in the runner.
- **In-flight prerequisite — already discharged.** The `prds-composition-integration` track (plans on CUDA, plan-based `sweep`/`compare`/`diff-backends`, composition widget) **completed on 2026-07-23** (`d38eb78`–`d52c892`, all five PRDs); `prds-project-hygiene` and `prds-demographic-slots` have both landed since. That track is the work that *closed* the capability gaps priority 1 names; A.0005 then generates the matrix that publishes the result. Any gap deliberately left open must be an explicit deterministic rejection before A.0005 runs. Preflight is therefore not blocked by this track — see the next comment for what does block it.

- **Preflight measurement (not a PRD).** Three commitments in this roadmap are quantitative and currently unmeasured: the Stage 1 claim that the CUDA plan path is load-bearing at ~27M slots, E.0001's predeclaration of a target geography resolution (which §5 makes the bound deciding whether M.0002 is viable and whether D.0003 is ever needed), and C.0002's corpus scope. All three are answered by the same artifact: the pending hardware rows in `docs/demographic-benchmark.md`, run with tooling that already exists (`scripts/bench-demographic.sh`, `sembla synth-state`). The scale-evidence run is therefore a Preflight prerequisite in the place the discharged composition-integration item used to occupy, and it precedes C.0001. It is evidence work, not a PRD: its deliverable is a committed evidence directory under `docs/evidence/demographic-bench/` and the resulting amendments to the scale note and E.0001's geography bound.

> [!warning] Review comment (2026-07-25) — The stated preflight dependency is stale; the real one is unmeasured scale
> The prerequisite above was satisfied on the day this document was drafted, and two full PRD tracks have landed since, so §0 currently gates Preflight on nothing. The dependency that *is* open is measurement: `docs/demographic-benchmark.md` carries four hardware rows (10M and 50M, CPU and CUDA) all marked **pending**, and the only measured point in the repository is 1M slots on a 16 GiB laptop. Two Stage-1 commitments rest entirely on extrapolation from that one point — the "Scale note" claim that the CUDA plan path is load-bearing at ~27M slots, and E.0001's predeclaration of a target geography resolution, which §5 makes the bound deciding whether M.0002 is viable at all and whether D.0003 is ever needed. Both are cheap to measure now with tooling that already exists (`scripts/bench-demographic.sh`, `sembla synth-state`) and expensive to unwind later. Recommend promoting the pending hardware benchmark to an explicit Preflight item, ahead of C.0001, and stating that E.0001's geography bound is chosen *from* it rather than in parallel with it.
>
> **Resolution (2026-07-25):** adopted, and the CPU half is now measured. The *Preflight measurement* item above replaces the discharged prerequisite; E.0001's geography bound is chosen from its output rather than in parallel with it (Stage 1). `docs/evidence/demographic-bench/local-2026-07-25/` extends the curve to 10M on the same machine class: **tick cost is linear in slots** (10× slots → 10.85× wall time), **artifact size is exactly 48 bytes/slot** across the range, and **peak RSS grows sublinearly** — 1.90 GiB measured at 10M against a 5.5 GiB linear projection from 1M. Time extrapolation is therefore supported: ~67 s/tick at 27M, a 13-minute 12-tick annual window. The 50M CPU row and both CUDA rows still require remote hardware; they are automated end-to-end under `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`, which collects all four remaining rows from one GPU VM and destroys it afterwards.

## 1. What should change about Sembla as a whole

Sembla's core thesis should **not** change: it remains a semantics-first simulation engine with one canonical IR/plan contract, Lean as the semantic authority, a deterministic CPU oracle, an accelerated backend, and reproducibility by construction. What should change is the roadmap's sequencing. The project has more designed capabilities than complete user workflows, so the next phase emphasises semantic closure, numerical trust, and usability before feature breadth.

A defensible software-wide priority order:

1. **Complete and publish the actual capability contract.** Audit legacy models versus linked plans, CPU versus CUDA, and `run` versus `sweep`, `compare`, and backend-differential workflows. Close the gaps or reject unsupported combinations explicitly — the closure work is the now-complete `prds-composition-integration` track (§0), not a new PRD here. The capability matrix must be *generated from the conformance test suite*—a hand-maintained matrix is the prose-drift problem it replaces, one layer up.
2. **Make numerical and model-assurance diagnostics first-class.** Practical `dt`-sensitivity and tau-leap convergence checks, saturation/capacity diagnostics, approximation-sensitivity reports, reusable model-card output, routine manifest-based reproduction. Sembla can establish what a model means and how it ran; it must not imply that reproducibility proves empirical validity.
3. **Add broadly reusable data-driven transition primitives.** Validated lookup and rate tables, time-indexed inputs, and fixed-coordinate categorical or conditional draws—with RNG coordinates, validation, and failure behaviour specified before implementation. Empirical data preparation stays external and enters through versioned, hash-audited artifacts.
4. **If evidence requires relational events, add one restricted atomic-event mechanism** rather than ad hoc cross-row writes: explicit participants, deterministic row or vacancy claiming, combined resource claims, all-or-none commit, stable event identity, and saturation/failure semantics. Birth allocation, matching, transfers, paired migration, and household moves should all reuse that one mechanism; generation-safe references accompany lifecycle-bearing links.
5. **Improve authoring and debugging before adding another backend or semantic family.** Reusable component libraries and templates, ergonomic table/matrix ingestion, validation errors that name the violated contract in user terms, observation-only trace/explain tooling, one canonical linker and execution semantics. At this stage this outranks diagnostics: Sembla's adoption risk is newcomers failing to get a first model running, not experts distrusting its numbers. **Schedule consequence (2026-07-25):** a stated priority that lands thirtieth in the execution order is not a priority, so the parts of this item with no second-driver dependency move to Preflight — C.0003 (validation errors) and C.0007 (trace/explain) both run before Stage 1. Only C.0004's worked examples stay late, because one of them *is* the justice pipeline and cannot precede Track J.
6. **Make the formal-semantics investment pay rent.** One end-to-end conformance chain from the Lean meaning through canonical plans to the CPU evaluator, backed by CPU/CUDA differential tests. Stabilise IR/CLI compatibility and artifact migration policy before v1 guarantees; time-box portable Level B determinism and document it honestly either way.

> [!note] Review comment — One priority-to-PRD gap remains
> The explicit serial schedule below resolves the earlier ambiguity around C/A ordering and gives authoring fixed checkpoints. Trace/explain is still promised here but has no owning PRD; add one or remove that promise so every stated priority maps to executable work.
>
> **Resolution (2026-07-24):** `C.0007-trace-explain-tooling` added to Track C, scheduled at the authoring checkpoint immediately after C.0004. *(Superseded 2026-07-25: moved to Preflight — it has no second-driver dependency and its value peaks while the baselines are being built.)*

Two governance rules qualify everything above:

- **Two-driver arbitration.** Priorities are pulled by real driver models with explicit validation targets and held-out checks—but a single driver risks overfitting the IR to one domain's idioms. Demand appearing in only one driver yields at most a provisional, single-driver PRD.
- **Evidence precedes its gate.** The work that *produces* gating evidence (comparison reports, design-options notes) must be scheduled before the gate, not behind it—otherwise nothing can ever pass it. This is the §K9 sequencing circularity, and it applies to every gate below.

**Product boundary (unchanged):** Sembla owns semantics, execution, diagnostics, canonical artifacts, and reproducibility; domain libraries own demographic or queueing concepts; external workflows own population construction, empirical rate estimation, NPE training, privacy treatment, and scientific validation datasets.

> [!warning] Review comment — Track ownership crosses this boundary later
> Track E later derives rates and generates entrant records, while A.0004 describes a fit-versus-held-out workflow. Clarify that Sembla's deliverables are schemas, validators, provenance/hash manifests, adapters, and reference fixtures; name the external owner and artifact hand-off for estimation, population construction, fitting, and scientific validation.
>
> **Resolution (2026-07-24):** for every Track E PRD, the *in-repo* deliverable is the artifact schema, validator, provenance manifest (defined once in E.0001), loader/adapter, and a reference fixture; the estimation and transformation scripts live in the external data workflow (today: the repository's `calibration/` pipeline plus the §G5 external `sbi` stack, both operated by the model owner) and hand off only versioned, hash-audited artifacts. Fitting itself is external per §G5; A.0004 consumes its outputs, it does not perform it. Scientific-validation datasets (held-out series) enter the same way.

## 2. Driver use case A — demographic population change model

A fixed-pool, slot-based aggregate accounting model of births, deaths, and migration over a national geography, run on monthly ticks with chained annual windows.

**Current status:** an executable accounting and software-validation fixture with a deterministic calibration-data pipeline—not a calibrated population model. The full limitations register is `scientific-limitations.md`; the load-bearing items are: births are a hazard on vacant slots, not on women (§2); internal migration is not identity-preserving and balances only in expectation (§3); there are no origin–destination matrices (§4); entrant characteristics are preclassified (§5); capacity, one-tick lockout, and simplified rates are unquantified approximations (§6–§8); and nothing is empirically initialised, fitted, or validated (§11–§12).

**What it demands, in ascending order of new semantics:**

1. Empirical initialisation and rates (ABS stocks by age × sex × area; mortality, fertility, migration rates).
2. Guardrails: automatic saturation rejection; quantified lockout and discretisation sensitivity; held-out validation.
3. Identity-preserving internal migration via a mutable area attribute and empirical origin–destination hazards—exact national balance by construction.
4. Externally generated entrant records; later, in-model categorical draws and time-indexed rate tables.
5. Women-at-risk births with vacant-slot claiming (first §K9-gated runtime construct; requires the design-options note first).
6. Generation-safe references, then household/family structures and synchronized household migration—strictly evidence-gated (§K9 trigger, Option D Phase 6).

**Validation targets:** observed stocks and flows, held-out age/sex/area trajectories, documented uncertainty and sensitivity, domain-expert review.

## 3. Driver use case B — justice system pipeline (queueing)

An **offending → courts → corrections pipeline**, modelled as a network of capacity-limited queues. It is policy-relevant in its own right—court backlogs, remand growth, prison capacity, sentencing and diversion reform—and structurally different enough from the demographic model to arbitrate which Sembla primitives are genuinely general.

**Model shape:**

- **Stations**: charges laid; remand and bail; court queues by court and matter type; sentencing; custodial and community corrections; release.
- **Flows**: lodgements feed court queues; courts dispose of matters at a rate limited by judicial sitting capacity; sentencing splits custodial versus community outcomes; custodial terms occupy corrections capacity for a near-fixed service time; releases return people to the community, with a recidivism hazard routing a share back to offending.
- **Policy levers**: court capacity and listing rules; sentence lengths; diversion programs; parole rates; prison bed numbers.
- **Decision-relevant outputs**: court backlog and waiting-time distributions; remand and sentenced populations against capacity; time-in-system; and the response of all three to capacity, sentencing, or diversion changes.

**Validation targets:** published lodgement/finalisation series, court backlog statistics, prisoner population series, time-served distributions, and recidivism rates (courts, corrections agencies, ABS).

## 4. How the two drivers arbitrate design decisions

The pair is useful precisely because they agree on some constructs and disagree on others:

| Design question | Demographic driver | Justice driver | Consequence for Sembla |
| --- | --- | --- | --- |
| Identity-preserving transfer | Internal migration as area change (§3) | Court → corrections station change | Same construct wanted by two unrelated domains → strong evidence for a general primitive (Track M) |
| Scheduler | Monthly ticks adequate | Dated events (hearings, releases) | Measure a staged-tick baseline first; event-driven scheduling only on measured distortion (J.0003) |
| Service-time law | Memoryless hazards | Near-deterministic delays (sentences) | Fixed-delay or Erlang/gamma staging; exercises linker nesting (Track D) |
| Capacity semantics | Storage ceiling (slot pools) | Contention (judges, beds) with blocking/backpressure | One canonical claim/contest contract must express both |
| Queue discipline | Not exercised | FIFO with deterministic tie-breaking; priorities; batch service (sitting days) | Extends the contest model under Level A determinism |
| Feedback | Weak (chained windows) | Strong endogenous loop (recidivism) | Composition wires with cycles |
| Observations | Grouped marginals | Sojourn- and waiting-time distributions | Observation IR beyond grouped counts (probes §14) |
| Kinship/households | Needed, §K9-gated | Not needed (individuals in queues) | Gate remains the demographic model's burden; justice must not smuggle kinship in |
| Data pattern | ABS stocks/rates artifacts | Courts/corrections/ABS series | Track E external-artifact pattern is domain-general |

Convergent demand (transfers, external rate artifacts, guardrails, fixed-pool state) is treated as corroborated and can proceed. Divergent demand (scheduler, contention semantics, queue disciplines) is exactly where the arbitration rule bites: no primitive lands on one driver's say-so alone.

## 5. Implementation roadmap, staged by PRD folder

Each workstream is a `docs/prds-<track>/` folder following the existing convention: numbered `NNNN-title.md` PRDs plus a `README.md` track index with objective, PRD status, and acceptance notes. PRDs are sized as single focused attempts with explicit acceptance criteria. Every gate is passed by committing a comparison report to the repository, not by judgement in the abstract.

> [!warning] Review comment — Make gates executable, not merely auditable
> A committed report records a decision but does not define pass/fail. Make §6 the sole normative gate definition and require each gate to predeclare its baseline and candidate, locked data and held-out split, metrics, quantitative threshold and uncertainty treatment, evidence DRI, independent approver, and `pass`/`fail`/`inconclusive` plus reopening rules. Each track README should also record its DRI, status, blockers, and next evidence artifact.
>
> **Resolution (2026-07-24):** §6 is now normative and carries the predeclaration rule. One deliberate divergence: no DRI or independent-approver roles. This is a single-runner repository, and standing-no #7 declines process machinery scaled for a project ~20× larger. The audit mechanism is instead structural: a gate's full predeclaration must land in a commit that *precedes* any evidence generation, so the git history — not a role — proves the threshold was not chosen after seeing the result. Track READMEs record status, blockers, and next evidence artifact per the existing convention.

**Construct-completeness rule (normative):** every PRD that adds semantics carries two acceptance criteria beyond its own behaviour.

1. **Backend story.** For each backend, exactly one of: *supported*; *deterministically rejected with a diagnostic naming the gap*; or *deferred to a named follow-up PRD that appears in the serial order below*. "Not mentioned" is not an available outcome. B.0001 additionally carries "the selected claim contract lowers to the DESIGN.md §4.2 closed fragment — map, filter, join-on-declared-keys, commutative-monoid group-by, segmented argmin, Philox-by-coordinate" as a named criterion, decided at design time while it is still free to change.
2. **Surface story.** Lean surface syntax, elaboration tests, negative tests, the parity check (`frontend/scripts/check-parity.sh`), and manifest recording where the construct is flagged — per §5.5's no-inert-syntax rule and the §K8 precedent. A construct reachable from the IR but not from the authoring surface is incomplete, not shipped.

A.0005's generated capability matrix remains the *detector* of drift; this rule is the *preventer*, and the two are not substitutes.

> [!danger] Review comment (2026-07-25) — Nothing here prevents a construct from landing half-supported
> Grouped observations are the precedent and the warning: a semantic construct landed CPU-only, and this roadmap now depends on it at national scale (see the Stage 1 comment above). The same failure is queued up twice more. Track B's "deterministic vacancy index" over a 27M-row table is a global allocation problem; if the claim contract selected in B.0001 does not lower to DESIGN.md §4.2's closed fragment (map, filter, join-on-keys, commutative-monoid group-by, segmented argmin, Philox-by-coordinate), it will be CPU-only by construction — and unlike grouped observations, it will be CPU-only in the *hot path* rather than the reporting path. Track D's lookup tables have the same exposure. A.0005's generated capability matrix is a detector, not a preventer: it publishes the drift one stage after it happens.
>
> Two cross-cutting rules would close this, and both are cheap enough to state here rather than re-litigate per PRD:
>
> - **Backend-parity declaration.** Every semantics-adding PRD declares, as an acceptance criterion, which of three states each backend is in — supported, deterministically rejected with a diagnostic naming the gap, or deferred to a *scheduled* follow-up PRD. "Not mentioned" stops being an available outcome. B.0001 additionally carries "the contract lowers to the §4.2 closed fragment" as a named criterion, decided at design time when it is still free to change.
> - **Constructs land complete through the surface.** Tracks D, B, R, and H are described runtime-first, but §5.5's no-inert-syntax rule and §K8's precedent mean each construct also needs Lean surface syntax, elaboration and negative tests, the parity check (`frontend/scripts/check-parity.sh`), and — where flagged — manifest recording. Left implicit, this produces IR capabilities the authoring surface cannot reach, which is precisely the failure mode priority 5 exists to prevent.
>
> **Resolution (2026-07-25):** both adopted verbatim as the construct-completeness rule above. The grouped-observations instance is handled separately in Stage 1, where the roadmap's dependency on it is concrete.

**Serial execution rule:** The PRD runner executes one PRD at a time, so only one PRD may be `Active`. The order below is the scheduling order even where adjacent PRDs have no technical dependency. “Independent” means the order may be changed only by explicitly revising this roadmap; it never means concurrent execution. If a gate does not pass, its locked PRDs are skipped or closed and execution continues with the next eligible item in the normative serial sequence. The Lean proof track is exempt from this rule (§0): it is opportunistic side work, never `Active` in the runner.

**Queue-ordering rule (normative):** the serial rule governs the *runner*. It does not govern the external evidence pipeline, which proceeds continuously and out of band. Measured runner throughput is roughly one PRD per hour of wall clock; external artifact acquisition, licensing, and evidence runs are slower by orders of magnitude, so a queue ordered by track systematically parks unblocked work behind blocked work. The runner queue is therefore ordered by **blocking resource**: within the stage ordering below, a PRD whose inputs are all in the repository outranks one waiting on an external artifact. The supporting artifact is a **data-dependency register** — one table naming each external artifact, its source, licence, vintage, acquisition status, and the PRDs it blocks — maintained in the Track E README and updated as artifacts land. A PRD whose register row is not `acquired` is not `Ready`, and the runner skips to the next eligible item rather than stalling.

> [!warning] Review comment (2026-07-25) — The serial rule is right; the ordering inside it queues fast work behind slow work
> The runner is not the bottleneck. Measured velocity in this repository is roughly one PRD per hour of wall clock: the nine `prds-demographic-slots` PRDs landed between `6806bea` (2026-07-24 01:08) and `7bf8bb0` (2026-07-24 10:47), and the five-PRD composition-integration track landed inside a day. The scarce resources are external and human — ABS artifact acquisition and licensing, transform-script authoring, evidence runs on hardware that is not yet provisioned, and gate predeclarations. This roadmap's 44 PRDs are therefore not a multi-month implementation programme; they are a few weeks of runner time wrapped around several months of data and evidence work.
>
> The within-stage order makes that coupling worse rather than better. Stage 1 states that "Track E is completed first; Track A follows even where an individual assurance PRD could technically have started earlier" — putting all six data-blocked PRDs ahead of Track A items that are blocked on nothing. When the runner reaches E.0001 without an ABS artifact in hand, the queue stalls with roughly forty hours of unblocked work sitting behind it.
>
> The fix preserves the serial rule exactly and changes only the queue order: **order the runner queue by blocking resource, not by track.** Data-free work (the C-track preflight, A.0001 and A.0003, D.0001, J.0001) runs while artifact acquisition proceeds out of band; E PRDs enter the queue as their inputs land. This needs one supporting artifact the roadmap does not yet have — a **data-dependency register** naming each external artifact, its source, licence, vintage, acquisition status, and the PRDs it blocks. Without it, "Track E is completed first" is a scheduling instruction whose feasibility nobody can see.
>
> **Resolution (2026-07-25):** adopted as the queue-ordering rule above. Stage 1's within-stage order is rewritten accordingly, J.0001 moves to Preflight (Stage 2), and the data-dependency register becomes a Track E README obligation with `Ready` conditioned on it.

### Stage 1 — Evidence foundations (data and assurance, no runtime semantics)

**In plain English:** This stage gives Sembla trustworthy ingredients and a dashboard of warning lights before anyone adds more engine parts. It replaces test-only demographic inputs with documented real-world data, checks whether time steps or capacity limits distort results, and publishes what the software actually supports. That enables a baseline people can inspect and compare; without it, later features could make the model more complicated without showing that it is more accurate or reliable.

**Within-stage serial order (revised 2026-07-25 under the queue-ordering rule):** A.0001 → A.0003 → A.0005 → E.0001 → E.0002 → E.0003 → E.0004 → E.0005 → E.0006 → A.0002 → A.0004 → baseline-adequacy gate. The three Track A items that consume no external artifact run first: A.0001 and A.0003 exercise the existing fixture, and A.0005's matrix is *generated* from the conformance suite, so it has no reason to wait for data — running it early publishes the current capability contract and the generator keeps it true thereafter. The E track then runs in dependency order as its artifacts land (E.0001 first regardless: it fixes the geography bound and the provenance schema every later artifact uses). A.0002 and A.0004 are genuinely data-blocked — both compare against the empirical baseline — and stay at the end, where the baseline-adequacy gate (§6) closes the stage.

**Data-consumption contract:** every Track E artifact targets only surfaces that exist today — `sembla.state/v1` initial-state artifacts, §K5 loaded rate-multiplier columns, and per-run θ across chained annual windows (§K4). E.0006 replaces the preclassified values *inside* the initial-state artifact; runtime RNG is untouched. An E PRD that cannot express its data on those surfaces (sub-annual variation, within-run rate changes) records the limitation in a representational-limits appendix instead of inventing semantics; those appendices are the evidence input to the Track D amendment gate (§6).

**Artifact-retention contract (added 2026-07-25):** §K4 makes every annual window a hashed `sembla.state/v1` artifact, and at ~48 measured bytes/slot a national-scale link is ~1.2 GiB — so a ten-year chain is ~12 GiB per parameter draw and a calibration sweep over it reaches terabytes. Intermediate chain links are therefore retained **as hashes, not as bytes**: the manifest tuples (`initial_state`/`exported_state`) and `verify-run` already reproduce any link from its manifest, so the reproduction path is the retention policy. Retained bytes are limited to the chain's first initial state, its final exported state, and any link a committed gate predeclaration names as evidence. Whichever PRD first runs a chained sweep at national scale states its disk budget explicitly.

> [!warning] Review comment (2026-07-25) — Chained windows have an unbudgeted storage cost at national scale
> §K4 makes chained annual runs the only way to vary rates between years, and each link is a hashed `sembla.state/v1` artifact. The measured artifact size is ~48 bytes/slot, so one national-scale link is ~1.2 GiB. A ten-year window chain is ~12 GiB per parameter draw; a 200-draw calibration over that chain writes ~2.4 TiB of intermediate state, and the manifest tuples that make the chain auditable (`initial_state`/`exported_state`) presuppose the artifacts still exist. Nothing in Stage 1 predeclares a retention policy. The cheap answer already exists in the repository — `verify-run` reproduces a run from its manifest, so intermediate links can be retained as hashes and regenerated on demand — but it has to be a stated contract before the first national-scale sweep, not a discovery during one. Recommend adding retention/regeneration to the data-consumption contract above, and a disk-budget line to whichever PRD first runs a chained sweep at scale.
>
> **Resolution (2026-07-25):** adopted as the artifact-retention contract above.

**Scale note (rewritten 2026-07-25):** national-scale initialisation (~27M slots) puts the CUDA plan path on the critical path for sweeps and calibration — but the driver model cannot reach it as built. Grouped observations are CPU-only (`crates/sembla-cli/src/main.rs:2219`), `diff-backends` refuses them outright, and the documented calibration workflow enables them. Stage 1 therefore splits the observation contract explicitly rather than discovering the constraint at A.0004:

- **Conditioning data `x` is scalar summaries.** Sweeps, calibration, and CRN contrasts run on declared scalar summaries only — the thin `(θ, x)` export §G5 already defines — and are CUDA-eligible at national scale. This is a restriction on what a *sweep* reports, not on what the model means.
- **Grouped marginals are validation outputs, not sweep outputs.** Age × sex × area cell counts are produced by a bounded number of CPU runs per selected θ, not once per draw. A.0004 budgets that CPU cost per candidate explicitly and states how many candidates the budget admits.
- **The §K9 CUDA-grouped deferral is decided, not left open.** The Preflight measurement resolves it: if the CPU cost of the bounded validation runs is acceptable at national scale, the deferral is closed with a recorded `DECISIONS.md` amendment saying so; if it is not, the follow-up folder is scheduled into Preflight and named in the serial order. Either outcome is recorded; "deferred and unscheduled while depended upon" is not.

Every Stage 1 PRD also ships a reduced-scale deterministic fixture so CI and goldens never depend on GPU hardware. C.0002's corpus scope inherits this split: the demographic model enters the differential corpus in its no-grouped configuration, and the grouped configuration is recorded as a known CPU-only path in A.0005's matrix rather than as an untested one.

> [!danger] Amendment (2026-07-25, measured) — the CUDA path is not merely restricted, it is unusable for this model class
> The split below assumes CUDA is the fast path for scalar-summary sweeps and that grouped observations are the only thing keeping the driver model off it. Measurement on one Hyperstack host (H100 PCIe + EPYC 9554, one binary, one commit, one seed, one state artifact, differing only in `--backend`) says otherwise:
>
> | 10M slots, 24 ticks, no-grouped | Wall time | s/tick |
> |---|---:|---:|
> | CUDA (H100 PCIe) | 1h 30m 08s | 225.3 |
> | CPU (EPYC 9554, same host) | 7m 19s | 18.3 |
>
> **The GPU is 12.3× slower than the CPU beside it.** The cause is identified, not inferred: four generated kernels (`sembla_validate_claims`, `_transition`, `_effects`, `_outputs`) validate every row on a *single* GPU thread, per claim and per fallible expression, per tick. The simulation kernels are properly parallel; the validation around them is serial. The loops are emitted only for models that contest a resource or dereference a `Ref`, which is why SIR reaches ~1,380 ticks/sec on the same hardware and why **every construct this roadmap plans next — vacant-slot claiming, generation-safe references, household links, identity-preserving migration — lands on the same path.**
>
> **Consequences for this document.** The scale note's option (b) survives as a *reporting* split but not as a performance strategy: there is currently no CUDA fast path for the driver model at all, so a national-scale calibration sweep is not merely inconvenient, it is infeasible on either backend until this is fixed. E.0001's geography bound is unaffected (it was released by the memory measurement, not the GPU one). C.0002's corpus scope is unaffected. What changes is that **§0's Preflight gains a prerequisite**: `docs/prds-cuda-validation-parallelism/` either lands or is explicitly closed before any stage depends on GPU throughput.
>
> **This is also a governance data point.** The roadmap's own construct-completeness rule (§5) asks every semantics-adding PRD to declare a backend story. Contests and `Ref` dereferences *had* one — CUDA "supported" — and it was true in the sense that results are correct. The rule as written would not have caught this, because it asks whether a backend works, not whether it is usable. Recommend extending it: a backend story that claims *supported* must cite a measurement, not a passing differential test.

**What the measurement settled (2026-07-25).** The CPU curve is now measured to 10M (`docs/evidence/demographic-bench/local-2026-07-25/`) and tick cost is linear in slots, so the extrapolation to national scale is supported rather than assumed: **~67 s/tick at 27M, ~13 minutes per 12-tick annual window**. That number cuts both ways, and it is the reason the split above is the right shape. A bounded set of validation runs — tens of candidates, each a few annual windows — is comfortably affordable on CPU, so grouped marginals do not need CUDA. A calibration sweep is not: 200 draws is roughly 44 hours and amortized NPE at 10³–10⁴ draws is out of reach, so scalar-summary sweeps must reach the GPU. Memory is no longer the constraint anyone thought it was — peak RSS at 10M measured 1.90 GiB against a 5.5 GiB linear projection — which removes the state-size objection to a finer geography and hands E.0001's bound back to the transition-count limit M.0002 documents. The CUDA rows remain unmeasured, so whether the GPU half of this split *works* is still open; that is what the remote collection answers.

> [!danger] Review comment (2026-07-25) — The CUDA path is closed for the driver model as built, and the scale note depends on it
> The demographic model's calibration and validation outputs are grouped views (`population_cells` by sex × area × five-year age, `deaths_cells`, `vacancy_cells`), and the documented calibration workflow in `docs/demographic-model.md` runs `sweep` with `--enable grouped-observations`. But grouped observations are CPU-only — `crates/sembla-cli/src/main.rs:2219` rejects them on CUDA ("grouped observations run on the cpu backend only for now"), and `diff-backends` refuses them outright (`main.rs:23`), which also means C.0002's differential corpus cannot cover the driver model in the configuration the driver model actually uses. §K9 defers "CUDA support for grouped observations" to a follow-up folder; this roadmap schedules no such folder, in any stage, while simultaneously making the CUDA plan path load-bearing for exactly those runs.
>
> The CPU fallback does not close the gap. Extrapolating the single measured point (1M slots, 24 ticks, 54.68 s, 551.8 MiB peak RSS) linearly to ~27M gives ≈61 s/tick, ≈12 minutes per 12-tick annual window, and ≈15 GiB resident — so a 200-draw sweep is ≈41 hours and amortized NPE at its usual 10³–10⁴ draws is out of reach. (The roadmap's own caution applies: this is extrapolation from one local run, which is itself the argument for the pending hardware rows.) *Measured 2026-07-25: the time figures were close — 67 s/tick and 13 min/window — and the memory figure was wrong by 3×, at 1.90 GiB measured at 10M. The conclusion stands: calibration sweeps need the GPU, validation runs do not.*
>
> Three exits, and Stage 1 should pick one before E.0001 rather than discover the constraint at A.0004: **(a)** schedule the §K9 CUDA-grouped follow-up folder into Preflight, making it a real prerequisite in place of the discharged one; **(b)** split the observation contract explicitly — scalar summaries are the conditioning data `x` for CUDA sweeps and calibration, grouped marginals are produced by a small number of CPU validation runs per selected θ — and state that split in the data-consumption contract, since it is a genuine and probably sufficient answer; or **(c)** fix a reduced operating scale for Stage 1 with a documented scale-invariance argument. Option (b) looks cheapest and costs no new semantics, but it constrains A.0004: held-out validation by age × sex × area then runs at CPU cost per candidate, which is a schedule fact worth writing down.
>
> **Resolution (2026-07-25):** option (b) adopted, with the §K9 deferral decided rather than left open — see the rewritten scale note above. The quantitative claims in this comment are being replaced by the §0 Preflight measurement; they were extrapolation from one point, which is what prompted the measurement.

- **Track E — `prds-empirical-demographics`**
  - `0001-empirical-initialization` — ABS observed stocks by age × sex × area, with a documented mapping into slot state. Also fixes two contracts every later PRD consumes: the **target geography resolution** (state/territory vs GCCSA vs SA4 — this bound decides whether M.0002's static origin–destination compilation stays viable and whether D.0003 is ever needed) and the **shared external-artifact provenance schema** (source, vintage, licence, transform-script hash) reused by E.0002–E.0006 and Track J's data audit. Three additions (2026-07-25): the geography bound is chosen *from* the §0 Preflight measurement, not in parallel with it; the provenance schema carries the geography **classification edition** (ASGS 2016 / 2021 / 2026) and the **concordance policy** applied when a stock vintage and a census edition disagree, which they routinely do; and the "documented mapping into slot state" states the within-year age assumption explicitly, since single-year-of-age stocks must become `age_months` and the uniform-within-year default is a choice, not a fact.
  - `0002-mortality-rate-tables` — age/sex/area mortality from empirical life tables. Owns two contracts the whole track then reuses (added 2026-07-25). **The temporal conversion:** published inputs are annual — life-table `qx` is an annual *probability*, ABS rates are annual occurrence-exposure rates — while §K5 columns multiply *hazards* and DESIGN.md §4.3 desugars via `λ = −ln(1−p)/Δt`. The PRD states where the conversion happens (external transform script, hash-audited, per the §0 product boundary) and in which form the artifact is expressed, with a test; A.0003 then quantifies the residual discretisation error against it. Substituting a probability for a rate misstates mortality by roughly `q²/2` per period — negligible at age 30, material where `q` approaches 0.2 and where projections are most sensitive. **The estimation method:** small-area death counts by single year of age are sparse and often suppressed, so area-level mortality is a smoothing or shrinkage problem, not a join; the method and its parameters are recorded as provenance fields rather than folded silently into the numbers, and A.0004's model card surfaces them as documented approximations.
  - `0003-fertility-scaling` — a defensible fertility derivation for the aggregate model, with approximation limits written down.
  - `0004-migration-rates` — observed immigration and emigration rates.
  - `0005-origin-destination-matrix` — census origin–destination tables as a validated rate artifact, with a documented **reconciliation** (added 2026-07-25). Census OD cells are perturbed for confidentiality and therefore do not sum to their own published marginals; a matrix compiled straight into per-(origin, destination) hazards inherits that non-additivity. The PRD rakes to published marginals and reports pre- and post-reconciliation residuals as provenance. This is a precondition for M.0003, not a refinement of it: M.0003 repurposes the balance residual as a defect detector on the assumption that its expectation is zero, and an unreconciled input matrix gives it a nonzero expectation from the data alone, masking exactly the defects it exists to catch.
  - `0006-entrant-record-generator` — externally sampled, correlated entrant characteristics (supersedes slot preclassification without touching runtime RNG).
- **Track A — `prds-model-assurance`**
  - `0001-saturation-guardrails` — automatic rejection of saturating parameter sets in sweep and calibration pipelines.
  - `0002-entrant-lockout-and-slot-allocation-sensitivity` — quantified effect of the one-tick protection period *and* of the initial slot allocation across strata (limitations §5–§6): two approximation-sensitivity analyses over the same empirical baseline, previously scoped as lockout only.
  - `0003-rate-discretization-checks` — `dt`-sensitivity and tau-leap convergence reports as first-class diagnostics.
  - `0004-validation-harness-and-model-card` — fit-versus-held-out workflow; generated limitations/model-card output.
  - `0005-capability-matrix` — generated from the conformance suite, replacing prose capability claims.

*Exit:* the **baseline-adequacy gate** (§6) resolves on an empirically initialised, guard-railed aggregate baseline with a published capability matrix and a generated model card. A `fail` does not stop the roadmap — it *directs* it: the per-output failure pattern is the cheapest evidence this plan can buy, and routes accordingly. Area-level stocks failing while national stocks pass is direct evidence for Track M; birth counts failing by maternal age is direct evidence for the Track B chain; failure across every output points at the rate artifacts rather than the semantics, and sends the runner back into Track E instead of forward into Stage 2.

> [!danger] Review comment (2026-07-25) — Stage 1 has no gate, and it is the stage most worth gating
> Every expensive construct downstream is gated, but the foundation everything else is built on is not: this exit criterion is a list of deliverables, not a pass/fail. A baseline that reproduces observed stocks to within 1% and one that misses by 10% both satisfy it, and both flow into Stage 2 unchanged. That inverts ordering principle 4, which says each stage's outputs decide whether the next is warranted — the principle is applied to births, references, and households, and skipped for the empirical baseline.
>
> Add a **baseline-adequacy gate** to §6 with the same predeclaration discipline as the others: locked data, a held-out year or years, metrics per decision-relevant output (age × sex × area stocks, national and area-level flows), a quantitative accuracy target, and replicate design. Its value is not gatekeeping — it is diagnosis. A baseline that fails on area-level stocks but passes nationally is direct evidence for Track M; one that fails on birth counts by mother's age is direct evidence for Track B; one that fails everywhere is evidence that the rate artifacts, not the semantics, are wrong. That is the cheapest possible way to earn the Stage 3 and Stage 4 gates' evidence, and this roadmap currently produces it by accident rather than by design.
>
> **Resolution (2026-07-25):** adopted. The baseline-adequacy gate is added to §6 with a replication design, and the Stage 1 exit is rewritten so a `fail` routes evidence to the stage that addresses it.

> [!warning] Review comment (2026-07-25) — Track E is missing the annual-to-monthly rate contract
> Published demographic inputs are annual: life-table `qx` is an annual *probability* of death, ABS fertility and migration rates are annual occurrence-exposure rates. The model runs monthly ticks, §K5 columns multiply *hazards*, and DESIGN.md §4.3 desugars probabilities via `λ = −ln(1−p)/Δt`. Nobody owns the conversion. E.0002 says "age/sex/area mortality from empirical life tables" and stops there, which leaves three unstated choices: whether conversion happens in the external transform script (hash-audited, per the product boundary) or through surface sugar in the model; whether an annual rate is applied as `q/12` or `−ln(1−q)/12`; and whether the tau-leap's tick-start rate freezing plus the one-tick entrant lockout bias the result further.
>
> This matters more than its size suggests. Substituting a probability for a rate misstates mortality by roughly `q²/2` per period — negligible at age 30, material in the oldest age groups where `q` approaches 0.2 and where population projections are most sensitive. Recommend: E.0002 owns an explicit, tested conversion contract stating where the conversion happens and in which direction the artifact is expressed; A.0003 quantifies the residual discretisation error against it, which is exactly what a `dt`-sensitivity report is for. The same contract then covers E.0003 and E.0004 rather than each re-deciding it.
>
> **Resolution (2026-07-25):** adopted into E.0002 above as the temporal-conversion contract, reused by E.0003 and E.0004.

> [!warning] Review comment (2026-07-25) — Track E assumes the artifacts are lookups when two of them are estimates
> E.0002 and E.0005 read as data-loading tasks; statistically they are not. Small-area death counts by single year of age are sparse and frequently suppressed, so area-level mortality is a smoothing or shrinkage problem, not a join — and the smoothing method becomes a load-bearing model assumption. Census origin–destination tables are perturbed for confidentiality, so their cells do not sum to their own marginals; a matrix compiled straight into per-(origin, destination) hazards inherits that non-additivity. That directly threatens M.0003's headline claim of "exact national balance **by construction**": the construction can be exact while the rates it is fed are not internally consistent, and the residual report — which M.0003 repurposes as a defect detector on the assumption that its expectation is zero — will then have a nonzero expectation from the input data alone, masking real defects.
>
> Under the §0 product boundary the estimation itself is external, which is right; the roadmap's job is to name the hand-off. Recommend: E.0005 requires a documented reconciliation (raking/IPF to published marginals) with pre- and post-reconciliation residuals reported; E.0002 requires the estimation and smoothing method recorded as provenance, not folded silently into the numbers; and E.0001's provenance schema gains two fields it currently lacks — the geography **classification edition** (ASGS 2016 vs 2021 vs 2026) and the concordance policy used when a stock vintage and a census OD edition disagree, which they routinely do. A.0004's model card then surfaces all three as documented approximations rather than leaving them in the transform scripts.
>
> **Resolution (2026-07-25):** all three adopted — reconciliation into E.0005, estimation-method provenance into E.0002, classification edition and concordance policy into E.0001.

> [!warning] Review comment — Stage 1 still needs an explicit data-consumption path
> The serial schedule now places C.0001/C.0002 before this stage, resolving A.0005's ordering dependency. Stage 1 still claims an executable empirical baseline before D.0001/D.0002 provide lookup and time-indexed input semantics: state whether Track E artifacts compile into existing loaded columns, static transitions, and per-run θ, or move the required input semantics earlier.
>
> **Resolution (2026-07-24):** answered by the data-consumption contract above — the existing surfaces (state artifacts, §K5 columns, per-run θ) are the compilation targets, chosen deliberately so Stage 1 adds zero runtime semantics; what they cannot express becomes recorded Track D evidence rather than a blocker.

### Stage 2 — Cheapest structural wins (both drivers)

**In plain English:** This stage makes the most useful improvements that can be built mostly with machinery Sembla already has. A person can change area without being destroyed and recreated, while the justice model is first built using ordinary time steps. That enables exact internal-migration accounting and gives us a working queueing baseline; we need that baseline so any request for new scheduling or identity machinery is backed by measured problems rather than guesses.

**Within-stage serial order (revised 2026-07-25):** M.0001 → M.0002 → M.0003 → (any prerequisite PRDs J.0001 added by explicit roadmap revision) → J.0002. **J.0001 has moved to Preflight** — it is the second driver's design and data audit, it adds no runtime semantics, it touches no goldens, and unlike every Track E item it waits on no artifact that must first be acquired. Leaving it here meant the contracts it exists to arbitrate (E.0001's provenance schema, Track M's transfer construct, A.0005's matrix) were all frozen as single-driver decisions before the second driver had spoken. The rest of the justice baseline stays in this stage, where it belongs.

- **Track M — `prds-identity-preserving-migration`** (depends on E.0005; no new identity machinery—codes, not references). M.0001 changes the demographic model's schema, so the demographic goldens are regenerated under the I6 policy with the change justified in the PRD; `examples/**` and every SIR golden stay byte-frozen.
  - `0001-area-as-attribute` — area as an enum attribute on the slot, changeable by transition effects; `(slot, generation)` identity untouched.
  - `0002-od-transition-generation` — origin–destination hazards from the empirical matrix; moves contest the person's own slot.
  - `0003-exact-balance-and-residual` — national internal balance by construction; residual report retained with expectation zero as a defect detector.

> [!question] Review comment — M.0002 needs an implementation-path decision
> E.0005 supplies an origin–destination artifact, not necessarily executable destination-selection semantics. Before M.0002 is ready, either depend on D.0001/D.0003 or state that the matrix is compiled externally into ordinary static per-destination transitions supported by the current IR; for the latter, document scalability limits and a plan-equivalence test.
>
> **Resolution (2026-07-24):** the static-compilation path is chosen. M.0002 compiles the E.0005 matrix externally into ordinary per-(origin, destination) transitions — guard on the origin, `set` the destination enum (§K8), contest the person's own slot (§K7) so at most one move wins per tick. No new IR. The PRD documents the scalability limit (transitions grow as areas × (areas − 1)); the path is viable only at or below the geography bound E.0001 predeclares. If E.0001 selects a finer geography than that bound, M.0002 is blocked pending D.0003 and this roadmap is revised — that is the explicit revision mechanism, not drift. If D.0003 ever lands, a plan-equivalence test against the compiled form is one of its named acceptance criteria.

- **Track J — `prds-justice-pipeline`, baseline half** (independent of Track M, but scheduled after M.0003)
  - `0001-model-design-and-data-audit` — **moved to Preflight (2026-07-25)**; listed here for continuity. Station structure, published data sources, explicit validation targets, **and a capability audit**: cyclic wiring at n-box scale (the recidivism loop), staged phase-type service times, per-entity observations, and the §J12-deferred heterogeneous scheduler domains. Each missing capability yields a prerequisite PRD added by explicit roadmap revision before J.0002; observation-IR extensions arising here are marked single-driver/provisional. Three amendments from the 2026-07-25 review:
    - **It now precedes E.0001 rather than following it**, so the provenance schema is designed against two consumers instead of one. Where the two drivers' needs conflict, E.0001 arbitrates and records why.
    - **The observation-capability item is restated.** The claim that grouped marginals cannot express sojourn and waiting times is too strong, and the overstatement is expensive — it points Stage 3 at new observation IR to buy something the current contract may already give. Grouped views take banded keys (`{"attr":"age_months","band_width":60}` in `fixtures/demographic/demographic_slots.json`) and §K8 gives arithmetic `set` effects, so a queue-entry-tick column plus a derived "ticks waiting" column plus a banded grouped view yields a waiting-time *distribution* as counts per bucket with no new semantics. The genuine gaps are **exact quantiles** rather than binned ones and **per-entity event streams** (§K9 defers those explicitly). J.0001 must build the banded-derived-column construction and show it insufficient before proposing any observation-IR extension — ordering principle 1 applies here as much as it does to area codes.
    - **It carries a data-availability finding with a predeclared fallback.** §3's validation targets assume published waiting-time and time-served *distributions*; court and corrections statistics are typically aggregate, reporting durations as medians and quartiles. J.0001 states which targets survive on aggregate quantiles alone and which are dropped, because J.0003's "predeclared tail tolerances" need an observable tail to be declared against, and discovering that at J.0005 would invalidate a gate already used to admit or reject three resource contracts.
  - `0002-tick-based-baseline` — method-of-stages queueing approximation on the existing tick runtime; document where it misleads (backlog dynamics, waiting-time tails) against analytical queueing cases (M/M/c, M/D/c) and an independent discrete-event oracle run on identical inputs. Those comparators are built here, so the J.0003 gate measures scheduler distortion in isolation rather than confounded with calibration and structural error.

> [!danger] Review comment (2026-07-25) — The arbitrating driver arrives after the contracts it is meant to arbitrate
> J.0001 sits at roughly position 19 in the serial order, but §4's whole argument is that a second, structurally unrelated driver is what separates general primitives from demographic conveniences in disguise. By the time J.0001 runs, E.0001 has frozen the external-artifact provenance schema that §5 explicitly says Track J's data audit reuses; M.0001–M.0003 have committed to an identity-preserving transfer construct that the table in §4 nominates as the strongest candidate for a general primitive on the strength of both drivers wanting it; and A.0005 has published a capability matrix. Each of those is a single-driver decision at the moment it is frozen, which is exactly what the two-driver rule exists to prevent — and the rule's own remedy ("demand appearing in only one driver yields at most a provisional, single-driver PRD") is never applied to them.
>
> J.0001 is a design and data-audit PRD. It adds no runtime semantics, touches no goldens, and — unlike every Track E item — depends on no artifact that must first be acquired. Moving it into Preflight, or immediately ahead of E.0001, costs one PRD slot of runner time and converts three frozen contracts from single-driver to corroborated. It also front-loads the justice data-availability risk (below), which is better discovered before Stage 1 than after Stage 2.
>
> **Resolution (2026-07-25):** adopted — J.0001 moves to Preflight, ahead of E.0001, and the Stage 2 order is rewritten accordingly.

> [!question] Review comment (2026-07-25) — J.0001's observation-IR claim is stronger than the repository supports
> J.0001 lists "per-entity sojourn/waiting-time observations (grouped marginals cannot express these — limitations §14)" as a capability gap that may generate a prerequisite PRD. That overstates the gap, and the overstatement is expensive: it points Stage 3 at new observation IR, marked single-driver/provisional, to buy something the current contract may already give.
>
> Grouped views take **banded** keys — `fixtures/demographic/demographic_slots.json` groups `age_months` with `band_width: 60` — and §K8 provides arithmetic `set` effects over full expressions. A queue-entry-tick column, plus a derived "ticks waiting" column, plus a banded grouped view over it, yields a waiting-time *distribution* as counts per bucket, with zero new semantics and full CUDA-parity exposure inherited from wherever grouped observations land (see the Stage 1 comment). What grouped marginals genuinely cannot express is per-entity **event streams** (§K9 defers those explicitly) and exact quantiles rather than binned ones — a real limitation, but a much narrower one, and one that predeclared tail tolerances in J.0003 may well tolerate. Recommend restating the audit item as "exact quantiles and per-entity event streams", and requiring J.0001 to test the banded-derived-column construction before proposing any observation-IR extension. Ordering principle 1 — buy interpretability with the least new semantics — applies here as much as it does to area codes and entrant tables.
>
> **Resolution (2026-07-25):** adopted into the J.0001 entry above.

> [!warning] Review comment (2026-07-25) — The justice driver's validation targets assume data that may not be published at that granularity
> §3 names "court backlog and waiting-time distributions" and "time-served distributions" as validation targets. Published court and corrections statistics are typically aggregate: finalisations and pending caseloads by court level and matter type, with duration reported as medians and quartiles rather than as distributions, and time-served often only as means by offence category. If that is what J.0001's audit finds, then J.0002's comparators can still be built (analytical M/M/c and M/D/c cases and the DES oracle are self-contained), but J.0005's held-out validation targets have to be restated in terms of what is actually published — and J.0003's "predeclared tail tolerances" may have no observable tail to be declared against. Recommend J.0001 carry an explicit data-availability finding with a predeclared fallback: which targets survive on aggregate quantiles alone, and which are dropped. Discovering this at J.0005 would invalidate a gate that has already been used to admit or reject three resource contracts.
>
> **Resolution (2026-07-25):** adopted — J.0001 now carries the data-availability finding and its predeclared fallback.

*Exit:* exact-balance migration in the demographic model; a working justice baseline whose approximation errors are measured.

### Stage 3 — Reusable data-driven primitives

**In plain English:** This stage teaches Sembla to use tables that say “the rate is X for this group at this time” and to make reproducible choices among several possible outcomes. It also uses the justice baseline to decide whether special scheduling and queue features are genuinely needed. This enables many models to use real, changing data without thousands of hard-coded rules, while protecting the engine from gaining expensive new semantics just because they sound useful.

**Within-stage serial order:** D.0001 → rate-semantics amendment gate → D.0002 and D.0003, each only if its `DECISIONS.md` amendment is accepted → J.0003 → per-contract gates → J.0004 / J.0006 / J.0007, each only if its contract gates in → J.0005 (unconditional). If a condition does not pass, close or skip the blocked item and continue to the next eligible checkpoint in the global sequence.

- **Track D — `prds-data-driven-transitions`** (RNG coordinate schemes specified and frozen before implementation; goldens regenerated only under proof)
  - `0001-lookup-rate-tables` — validated lookup/rate tables as plan inputs. New IR, but in conflict with no adopted decision; still records its own `DECISIONS.md` entry before implementation.
  - `0002-time-indexed-rates` — within-run time-indexed rate tables. Requires an accepted amendment to §K5.
  - `0003-categorical-draws` — fixed-coordinate categorical/conditional draws for destinations, entrant attributes, and policy choices. Requires an accepted amendment to §K9.

> [!danger] Review comment — Track D conflicts with adopted decisions as written
> `DECISIONS.md` §K5 rejects time-indexed rate-table semantics for now, and §K9 defers categorical draws and rejects advancing deferred constructs without their named trigger. D.0002/D.0003 are unconditional here and no decision-amendment step exists. Add the evidence and explicit `DECISIONS.md` amendment required before either PRD can become ready.
>
> **Resolution (2026-07-24):** a rate-semantics amendment gate now precedes D.0002/D.0003 (§6). Its evidence note is assembled from three sources already scheduled ahead of it: Track E's representational-limits appendices (what could not be expressed as loaded columns or per-run θ), M.0002's documented static-compilation bound, and J.0001's capability audit. The note proposes explicit amendments — to §K5 for time-indexed tables, and to §K9 for categorical draws, whose current placement under the *identity-linkage* trigger is the wrong trigger for destination/routing draws and should be given its own (geography exceeding the static bound, or corroborated justice routing demand). Each amendment is accepted or rejected in `DECISIONS.md`; a rejected amendment closes its PRD and the serial order continues.

- **Track J — gate and conditional primitives**
  - `0003-scheduler-decision-report` — the scheduler gate: compares the staged-tick baseline against the J.0002 analytical/DES comparators on identical inputs, with predeclared tail and backlog tolerances. The candidate constructs are layered, cheapest first: phase-type staging (no new semantics — the J.0002 baseline itself), §C6 scheduled clocks (already designed; tick-quantised firing dates with guard-recheck), and true next-event execution (new semantics, additionally gated on the Option D Phase 8 heterogeneous-scheduler-domain decision that `docs/ROADMAP.md` v0.5 already requires). A negative result keeps the runtime tick-only and *defers* §C6's construct — it does not un-adopt that design. The report also gates each resource contract separately (next item).
  - `0004-server-pool-primitives` — only if its own gate line passes: server pools with acquire/release under Level A (judicial sitting capacity, prison beds). Single-driver/provisional; promotion requires an independent conformance case from a second domain (e.g., a hospital-ED or call-centre example reusing the same contract), named in the PRD.
  - `0005-validation-and-model-card` — **unconditional**: backlog and prisoner-population trajectories against held-out published series; generated limitations/model-card output per A.0004. Runs whatever the scheduler and resource decisions were.
  - `0006-blocking-and-backpressure` — only if its own gate line passes: blocking/backpressure semantics (remand grows when beds are full). Single-driver/provisional, same promotion rule as J.0004.
  - `0007-queue-disciplines` — only if its own gate line passes: FIFO with deterministic tie-breaking, priorities, batch service (sitting days) under Level A. Single-driver/provisional, same promotion rule.

> [!warning] Review comment — The justice decision lacks prerequisites and a valid comparator
> Observed justice series alone cannot isolate scheduler distortion from calibration and structural error. J.0001 should audit cyclic composition, staged service times, and per-entity sojourn/waiting-time observations, creating prerequisite PRDs for missing capabilities; J.0003 should compare identical inputs against analytical queue cases or an independent discrete-event oracle with predeclared tail/backlog tolerances. Add held-out validation and model-card work regardless of the scheduler outcome.
>
> **Resolution (2026-07-24):** all three adopted — J.0001 now carries the capability audit with a prerequisite-PRD mechanism, J.0002 builds the analytical/DES comparators, and J.0005 (validation and model card, present in the original track list but dropped from this document's first cut) is restored as unconditional.

> [!warning] Review comment — Scheduler and queue semantics need separate gates
> A negative scheduler result says ticks are adequate; it does not show that acquire/release resources, blocking/backpressure, or deterministic FIFO are unnecessary. Split J.0004 into focused PRDs and gate each semantic contract on its own measured need. Under the two-driver rule, mark justice-only queue contracts `single-driver/provisional` and define the independent conformance case required to promote them.
>
> **Resolution (2026-07-24):** adopted — J.0004 is split into J.0004/J.0006/J.0007, each behind its own gate line in the J.0003 report, each marked single-driver/provisional with a named promotion condition.

*Exit:* the general primitives both drivers need; a documented scheduler decision; a held-out-validated justice model card regardless of that decision.

### Stage 4 — §K9-gated runtime constructs (demographic driver only; strictly sequential)

**In plain English:** This stage adds the difficult machinery needed when one event must safely affect more than one record—for example, a woman giving birth into a vacant slot or a whole household moving together. It enables meaningful parent and household links plus coordinated changes, but only after evidence shows that simpler aggregate models are not enough. The gates matter because mistakes here could create stale references, half-completed updates, or results that are no longer deterministic.

**Within-stage serial order:** A.0006 → B.0001 → birth gate → B.0002 → B.0003 → B.0004 → A.0007 → reference gate → R.0001 → R.0002 → R.0003 → A.0008 → household gate → H.0001 → H.0002 → H.0003. Each gated block runs only after its gate passes; if a gate fails, that block and every downstream block that depends on it are closed and the runner proceeds to the final Track C checkpoints.

- **Track A — Stage 4 evidence PRDs** (scheduled here, not in Stage 1, so each report is produced against the then-current baseline and lands immediately before the gate it feeds; all three are evidence-only, with zero runtime change)
  - `0006-maternal-exposure-evidence` — recompute births under a women-at-risk normalisation (externally or as an aggregate calculation over the same empirical rates) versus the vacant-slot formulation, on locked data with a predeclared materiality threshold over decision-relevant outputs. Feeds the birth gate.
  - `0007-linkage-sensitivity-evidence` — externally imposed linkage between chained windows (limitations §13, option 5) or an aggregate proxy; with-versus-without comparison on decision-relevant outputs. Feeds the reference gate.
  - `0008-household-linkage-sensitivity` — household-level sensitivity (synchronised moves, household-composition outputs), distinct from genealogy. Feeds the household gate.

- **Track B — `prds-women-at-risk-births`** (gate: A.0006 evidence that maternal exposure materially changes decision-relevant outputs, plus the note)
  - `0001-design-options-note` — required by §K9; records the evidence and chosen claim semantics. Scheduled *before* the gate it informs. Two further obligations: **(a)** it selects and versions the *single* restricted atomic-event contract §1 promises — participants, event identity, claim ordering, combined claims, all-or-none commit, RNG coordinates, saturation and failed-claim semantics — which B.0002–0004, Track R, and Track H then consume rather than inventing mechanisms, resolving H.0003's direct-write-versus-event fork here rather than at the end; and **(b)** it proposes the §K9 amendment giving claim-only births their own trigger, distinct from full identity linkage, to be accepted or rejected in `DECISIONS.md` before B.0002 can be `Ready`.
  - `0002-vacant-slot-claiming` — deterministic vacancy index; claim on a general vacant person slot.
  - `0003-atomic-two-row-commit` — hazard fires on an eligible woman; child slot claimed and initialised; all-or-none.
  - `0004-saturation-and-failed-claim-semantics` — defined behaviour at capacity, with new goldens.

> [!danger] Review comment — The birth gate both conflicts with K9 and lacks an evidence producer
> `DECISIONS.md` §K9 names identity linkage—not maternal-exposure improvement—as the trigger for vacant-slot claiming and cross-row writes. No scheduled pre-gate task constructs the maternal-exposure counterfactual that the current row-local runtime cannot express. Add an evidence-only external or aggregate women-at-risk comparison with locked data, metrics, and a materiality threshold, then either amend §K9 to define a separate trigger for claim-only births or retain its existing gate.
>
> **Resolution (2026-07-24):** both halves adopted — A.0006 is the scheduled evidence producer, and B.0001 obligation (b) is the explicit §K9 amendment step. The birth gate cannot pass without the accepted amendment.

- **Track R — `prds-generation-safe-references`** (gate: Track B operating, and A.0007 evidence that permanent linkage is required)
  - `0001-ref-generation` — `Ref` carries or validates generation.
  - `0002-validator-loader-state-format` — validation, loading, canonical-hashing updates.
  - `0003-non-exclusive-mutable-refs` — aliasing, validity after export/reload, claim-accounting semantics.
- **Track H — `prds-kinship-and-households`** (gate: §K9 trigger met by the A.0008 report; Option D Phase 6 alignment)
  - `0001-household-table-and-mother-linked-births` — fixed household table; generation-safe `mother_ref`/`household_ref`.
  - `0002-synchronized-household-migration` — combined claims across member rows; households move as one event.
  - `0003-event-mediated-resolution` — explicit event tables and resolver, only if B.0001's atomic-event contract selected event mediation; otherwise closed at B.0001 time, not left as a late fork.

> [!danger] Review comment — The linkage gate is circular
> Track B creates no permanent genealogy, so “B + comparison reports” cannot produce a with-versus-without-linkage comparison, and Track H cannot supply evidence for the gate that blocks it. Add a pre-R evidence task using externally imposed linkage or an aggregate proxy, and require a separate household-linkage sensitivity report before Track H.
>
> **Resolution (2026-07-24):** adopted — A.0007 (pre-reference-gate, externally imposed linkage) and A.0008 (pre-household-gate sensitivity) break the circularity; both are scheduled in the serial order above and named in §6.

> [!warning] Review comment — Define the one atomic-event contract before its specialisations
> §1 promises one reusable restricted mechanism, but B.0003 introduces a birth-specific commit while H.0003 leaves direct writes versus event mediation as an optional late fork. B.0001 should select and version the shared participant, event-identity, claim-ordering, combined-claim, all-or-none commit, RNG, saturation, and failed-claim contract; B and H should then consume that contract rather than inventing separate mechanisms.
>
> **Resolution (2026-07-24):** adopted as B.0001 obligation (a); B.0003 becomes the contract's first consumer, and H.0003's fork is resolved inside B.0001.

*Exit at each sub-stage:* the comparison report either justifies the next construct or closes the ladder—both are legitimate outcomes.

### Cross-cutting checkpoints — Track C, `prds-conformance-and-authoring`

**In plain English:** This workstream is the safety rail that supports every stage. It checks that Sembla's formal meaning, CPU implementation, and accelerated implementation agree; improves errors and examples; and defines compatibility rules. It is needed so each new feature is understandable, testable, reproducible, and safe to carry forward instead of becoming another partly supported path.

**Serial checkpoint order (revised 2026-07-25):** C.0001 → C.0002 → C.0003 → C.0007 run before Stage 1, after the §0 Preflight measurement and J.0001; C.0004 runs after Stage 3, including whichever conditional J PRDs were activated; C.0008 runs before C.0005; C.0005 → C.0006 run after Stage 4, or after its gated branches are formally closed. Track C is cross-cutting in purpose, but its PRDs never run concurrently with another track.

- `0001-end-to-end-conformance-chain` — one chain from the Lean meaning through canonical plans to the CPU oracle.
- `0002-cpu-cuda-differential-ci` — backend differential testing under the recorded §J14 evidence discipline: local criteria (compilation, corpus listing, graceful skips, legacy goldens) run in hosted GPU-less CI; the hardware corpus runs via the tracked remote runbook with its evidence committed. The PRD must not promise GPU-in-CI that the infrastructure cannot deliver.
- `0003-validation-error-contract` — errors name the violated contract in user terms.
- `0004-component-library-and-worked-examples` — one polished end-to-end example per semantic family; the justice pipeline (Track J) is the named second driver, de-risking overfitting the IR to demography.
- `0005-ir-cli-compatibility-policy` — artifact migration policy before any v1 guarantees.
- `0006-portable-determinism-report` — time-boxed Level B feasibility, documented honestly either way.
- `0007-trace-explain-tooling` — observation-only trace/explain (which rule fired, which contest won and why, per tick), with a test that enabling it leaves the run's state hash bitwise unchanged per the §4.6 invariant. Numbered out of schedule order to keep C.0005/C.0006 references stable. **Moved to Preflight 2026-07-25:** it has no second-driver dependency, and its value peaks while the empirical and queueing baselines are being built — debugging why a rule did not fire at 27M rows without a trace is the concrete form of the adoption risk priority 5 names. The §4.6 invariance test makes it cheap to accept.
- `0008-legacy-plan-convergence-policy` — **added 2026-07-25.** The repository runs two execution paths with materially different capability: `compare` rejects mixed arms (§J14.3), sweeps carry `PlanIdentityTuple` while legacy sweeps carry `ir_hash` (§J14.4), `--dt` applies to one and is rejected for the other (§J14.5), and `diff-backends` grew a sibling `--all-plan-fixtures` flag rather than one corpus (§J14.6). Each decision is individually correct; together they are a fork, and priority 5 asks for one canonical linker and execution semantics. This PRD states which path is canonical for new work, whether the legacy path is frozen-for-goldens or deprecated with a migration, and which capability-matrix differences are permanent by design versus tracked as gaps. It runs before C.0005 because a compatibility policy written over an unresolved fork freezes two contracts at v1 instead of one.

No widening of the user base occurs before C.0003 and C.0004 have both completed.

> [!warning] Review comment (2026-07-25) — §1 ranks authoring above diagnostics; the schedule does the reverse
> Priority 5 states plainly that "at this stage this outranks diagnostics: Sembla's adoption risk is newcomers failing to get a first model running, not experts distrusting its numbers." The serial order then places the entire diagnostics programme (A.0001–A.0005) at positions 7–11 and the authoring checkpoint (C.0004, C.0007) at roughly position 30, behind two full stages and three gates. Only C.0003 — validation errors — lands early. That is a straight inversion of a stated priority, and the document does not acknowledge it as a deliberate trade.
>
> There is a defensible reason for part of it: C.0004 wants one polished example per semantic family, and the justice example cannot exist before Track J. But C.0007's trace/explain tooling has no such dependency, and its value is highest exactly while someone is building the empirical baseline and the queueing baseline — debugging why a rule did not fire at 27M rows without a trace is the concrete form of the adoption risk priority 5 names. Recommend either moving C.0007 to Preflight beside C.0003 (it is observation-only, with a state-hash-invariance test that makes it cheap to accept), or amending priority 5 to say that authoring outranks diagnostics *in principle* while the second driver's example blocks its delivery. Either is fine; the current silent inversion is what undermines the priority list's authority elsewhere.
>
> **Resolution (2026-07-25):** the first option adopted — C.0007 moves to Preflight beside C.0003, and priority 5 gains an explicit schedule-consequence sentence saying why C.0004 alone stays late.

> [!warning] Review comment (2026-07-25) — Track C never resolves the legacy/plan duality it inherits
> Priority 5 asks for "one canonical linker and execution semantics" and warns against "a proliferation of partially compatible authoring or execution paths". The repository already has exactly one such proliferation, and it is load-bearing: legacy models and plan envelopes are two execution paths with materially different capability. `compare` rejects mixed arms deterministically (§J14.3), sweeps carry `PlanIdentityTuple` while legacy sweeps carry `ir_hash` (§J14.4), `--dt` applies to one and is rejected for the other (§J14.5), and `diff-backends` grew a sibling `--all-plan-fixtures` flag rather than one corpus (§J14.6). Each of those decisions is individually correct; together they are a permanent fork.
>
> A.0005 will faithfully publish that fork as a two-column capability matrix, and C.0005 will then write a compatibility policy over it — which is how a v1 ends up freezing two contracts instead of one. Recommend Track C own an explicit convergence policy before C.0005: which path is canonical for new work, whether the legacy path is frozen-for-goldens or deprecated with a migration, and what the capability matrix is permitted to show as a permanent difference versus a tracked gap. This is a one-PRD decision now and a compatibility-guarantee problem later.
>
> **Resolution (2026-07-25):** adopted as C.0008 above, scheduled before C.0005.

### Normative serial execution order

```text
Preflight (composition-integration prerequisite discharged 2026-07-23, §0):
★ scale-evidence measurement (not a PRD; decides the geography bound,
  the §K9 CUDA-grouped deferral, and the C.0002 corpus scope)
  [DONE 2026-07-25: geography bound released — peak RSS is sublinear, 1.90 GiB
   at 10M against a 5.5 GiB linear projection. CUDA measured 12.3x SLOWER than
   the same host's CPU; see the Stage 1 amendment.]
→ prds-cuda-validation-parallelism (lands or is explicitly closed before any
  stage depends on GPU throughput)
→ J.0001 (second driver on the table before the shared contracts freeze)
→ ★ C.0001 → ★ C.0002 → ★ C.0003 → C.0007

Stage 1 (ordered by blocking resource, not by track):
★ A.0001 → ★ A.0003 → ★ A.0005          [no external artifact required]
→ ★ E.0001 → ★ E.0002 → ★ E.0003 → ★ E.0004 → ★ E.0005 → E.0006
→ ★ A.0002 → ★ A.0004 → ★ baseline-adequacy gate

Stage 2:
★ M.0001 → ★ M.0002 → ★ M.0003
→ (prerequisite PRDs J.0001 added by roadmap revision, if any)
→ J.0002

Stage 3:
D.0001 → rate-semantics amendment gate
→ D.0002 only if the §K5 amendment is accepted
→ D.0003 only if the §K9 amendment is accepted
→ J.0003 → per-contract gates
→ J.0004 / J.0006 / J.0007, each only if its contract gates in
→ J.0005 (unconditional)

Authoring checkpoint:
C.0004

Stage 4:
A.0006 → B.0001 → birth gate
→ B.0002 → B.0003 → B.0004 only if the birth gate passes
→ A.0007 → reference gate
→ R.0001 → R.0002 → R.0003 only if the reference gate passes
→ A.0008 → household gate
→ H.0001 → H.0002 → H.0003 only if the household gate passes

Final guarantees:
C.0008 → C.0005 → C.0006
```

This is a total scheduling order, not a claim that every adjacent pair has a technical dependency. Conditional PRDs are skipped when their gate does not pass; no later PRD starts early merely because it is independent.

**★ marks the minimum spine (added 2026-07-25).** The spine is the subset that constitutes a defensible, publishable result standing alone: an empirically initialised, guard-railed, exactly-balanced aggregate baseline with a published capability matrix, a generated model card, and a resolved adequacy gate. Everything unmarked is an extension. §6 requires each gate to predeclare its threshold before seeing the result, on the argument that a threshold chosen afterwards proves nothing; the same argument applies to scope. If the external data or evidence work runs out of road at position 14, the decision about what to ship should already have been made — a truncation at the spine is a plan being followed, not a plan being abandoned. E.0005 is inside the spine only because M.0002 compiles it; E.0006 sits just outside deliberately, because preclassified entrants are a recorded limitation the model card can carry honestly.

> [!question] Review comment (2026-07-25) — Name the minimum spine before the order is executed
> This sequence is 44 PRDs across nine tracks, with five gates and three `DECISIONS.md` amendments. Its runner cost is modest (see the velocity comment in §5); its external-evidence cost is not, and that is the part that can stall. §6 requires every gate to predeclare its threshold before seeing the result, on the sound argument that a threshold chosen afterwards proves nothing. The same argument applies one level up: if the data or evidence work runs out of road at position 14, the decision about what to ship is made under pressure, after the fact, exactly as a post-hoc threshold would be.
>
> Recommend predeclaring a **minimum spine** — the subset that constitutes a defensible, publishable result on its own — and marking each remaining PRD as spine or extension. A plausible spine is the Preflight C-track, E.0001–E.0004, A.0001–A.0005, and M.0001–M.0003: an empirically initialised, guard-railed, exactly-balanced aggregate baseline with a published capability matrix and model card, which is a genuine scientific improvement over the current fixture and stands without any Stage 3 or Stage 4 outcome. Everything after it is then explicitly optional in the schedule as well as in the gate register, and a truncation is a plan being followed rather than a plan being abandoned.
>
> **Resolution (2026-07-25):** adopted — the spine is marked in the normative order below.

### Ordering principles

1. Buy scientific interpretability with the least new semantics in each step—area codes before references, external entrant tables before in-model draws, aggregate baselines before kinship, staged ticks before a new scheduler.
2. Two structurally contrasting driver models arbitrate which primitives are genuinely general; single-driver demand yields at most a provisional PRD, marked single-driver.
3. The design-options note (or decision report) precedes gated runtime work; the note is what evaluates the evidence, so it cannot itself sit behind the gate.
4. Each stage's outputs decide whether the next, more invasive stage is warranted; a negative result at any gate (linkage changes nothing; staged ticks reproduce backlog dynamics adequately) is a legitimate, cheap finding and closes the ladder.
5. PRDs are sized as single focused attempts with explicit acceptance criteria; each folder's README tracks status so this roadmap can drift-correct without re-litigating scope.
6. The product boundary holds throughout: Sembla owns semantics, execution, diagnostics, and canonical artifacts; data preparation, rate estimation, fitting, and scientific validation remain external and enter only as versioned, hash-audited artifacts.
7. *(Added 2026-07-25.)* A construct is not shipped until it is complete in both directions — a declared story for every backend, and a reachable path from the authoring surface. Detection after the fact (the generated capability matrix) is not a substitute for prevention at design time.
8. *(Added 2026-07-25.)* The serial rule governs the runner, not the world. Order the queue by blocking resource so that fast, unblocked work never waits behind slow, externally blocked work; the external evidence pipeline runs continuously alongside it.

## 6. Gate register (normative)

This register is the sole normative definition of every gate; prose elsewhere in this document is commentary. Before a gate's evidence run starts, its full predeclaration must be committed as `docs/gates/NNNN-<gate>.md`, **in its own commit, preceding any evidence generation** — a predeclaration authored inside the evidence PRD lands in the same commit as the evidence and the audit trail evaporates, since the runner commits per PRD. It states: baseline and candidate; locked input data and held-out split; metrics; quantitative threshold; the **replication design**; and the reopening rule.

**Replication design (added 2026-07-25).** Every gate here compares stochastic runs, so "uncertainty treatment" is specified rather than left to the PRD: the predeclaration states the replicate count and seed policy (common random numbers for baseline-versus-candidate contrasts per §E5; independent noise where the quantity of interest is a variance), the estimated Monte-Carlo standard error of each metric at that replicate count, and a materiality threshold that exceeds it by a stated factor. Without this the failure is predictable and one-directional: a threshold set below Monte-Carlo error makes `inconclusive` the natural outcome, the one-follow-up rule converts it to `fail`, and a construct is closed on noise rather than evidence. A.0006's maternal-exposure comparison is the clearest exposure — a birth-count difference of a few percent at fixture scale is well inside run-to-run variation. Where the threshold cannot be made to exceed the Monte-Carlo error at achievable cost, the honest predeclaration says the gate is underpowered, and that is itself a finding worth committing. Every gate resolves to `pass`, `fail`, or `inconclusive`; `inconclusive` permits exactly one predeclared follow-up run, after which the outcome is `fail`. A failed or closed gate reopens only via a new committed predeclaration citing new evidence. (No DRI/approver roles: single-runner repository, standing-no #7 — the commit ordering is the audit trail.)

| Gate | Evidence required | Produced by | Unlocks |
| --- | --- | --- | --- |
| **Baseline adequacy** (added 2026-07-25) | Empirically initialised baseline reproduces held-out observed stocks and flows within predeclared per-output accuracy targets, at a predeclared replicate count | E.0001–E.0005 + A.0002 + A.0004 | Stage 2. A `fail` is diagnostic, not terminal: the per-output failure pattern routes to Track M (area-level stocks), the Track B evidence chain (births by maternal age), or back into Track E (failure across every output) |
| Rate-semantics amendments (§K5, §K9-draws) | Representational limits Track E could not express; M.0002 static-compilation bound vs the E.0001 geography; J.0001 audit; accepted `DECISIONS.md` amendment per construct | E appendices + M.0002 + J.0001 → amendment note | D.0002 (§K5 amendment), D.0003 (§K9 amendment), each independently |
| Scheduler semantics | Staged-tick distortion vs J.0002 analytical/DES comparators on predeclared tail/backlog tolerances | J.0002 + J.0003 | §C6 scheduled clocks first; next-event execution additionally requires the Option D Phase 8 decision |
| Justice resource contracts (three separate lines: server pools; blocking/backpressure; queue disciplines) | Measured need per contract in the J.0003 report; single-driver marking with named promotion condition | J.0003 | J.0004 / J.0006 / J.0007 respectively |
| Vacant-slot claiming / atomic-event contract | Maternal exposure materially improves decision-relevant outputs; §K9 amendment for a claim-only trigger accepted; design-options note | A.0006 + B.0001 | B.0002–0004 |
| Generation-safe references | Externally imposed linkage sensitivity shows permanent linkage is required; Track B operating | A.0007 | Track R |
| Kinship / households | §K9 trigger: household-linkage sensitivity shows identity linkage scientifically required | A.0008 + design-options note | Track H (Option D Phase 6) |
| Portable Level B determinism | Time-boxed feasibility investigation | C.0006 | Documented guarantee or honest limitation |
| v1 compatibility promises | Conformance chain + differential evidence green (per the §J14 discipline); migration policy written | C.0001, C.0002, C.0005 | Public IR/CLI stability guarantees |

> [!danger] Review comment (2026-07-25) — Every gate compares stochastic runs, and the register never says how many
> The predeclaration list requires "quantitative threshold and uncertainty treatment", which is the right words in the wrong resolution. Each of these gates compares simulation output against a baseline or against observed data, and every one of those comparisons has Monte-Carlo error that the register leaves to the individual PRD. Without a replication design the failure is predictable and one-directional: a materiality threshold set below the Monte-Carlo standard error makes `inconclusive` the natural outcome, the one-follow-up rule then converts it to `fail`, and a construct gets closed on noise rather than on evidence. A.0006's maternal-exposure comparison is the clearest exposure — a birth-count difference of a few percent at the fixture's scale is well inside run-to-run variation.
>
> Recommend the predeclaration additionally state: the replicate count and seed policy (common random numbers for baseline-versus-candidate contrasts, per §E5 — independent noise where the quantity of interest is the variance itself); the estimated Monte-Carlo standard error of each metric at that replicate count; and a requirement that the materiality threshold exceed it by a stated factor. Where it cannot, the honest predeclaration is that the gate is underpowered at achievable cost, which is itself a finding worth committing.
>
> **Resolution (2026-07-25):** adopted into the §6 predeclaration list.

> [!warning] Review comment (2026-07-25) — The audit mechanism needs a location and a template to be real
> §6's resolution of the DRI question is convincing: commit ordering, not a role, proves the threshold predates the result. But nothing says *what file* a predeclaration is committed to, and the PRD runner commits per PRD — so a predeclaration authored inside the evidence PRD lands in the same commit as the evidence, and the audit trail silently evaporates. `docs/evidence/` already exists as a convention (`docs/evidence/demographic-bench/`), which gives the shape: add `docs/gates/NNNN-<gate>.md` with a template covering the eight predeclared fields, committed on its own before the evidence PRD becomes `Active`, and amended only by a subsequent commit citing new evidence. One paragraph here turns the mechanism from an intention into something a reader can check with `git log`.
>
> **Resolution (2026-07-25):** adopted — `docs/gates/NNNN-<gate>.md` is now the committed location, named in §6.

## 7. Relationship to the limitations register

`scientific-limitations.md` remains the record of what the current demographic fixture is not. This roadmap is the plan for changing that—and for making Sembla a tool whose semantics, diagnostics, and artifacts two different scientific domains can both build on without either one's idioms becoming the framework's assumptions.

**The register becomes machine-readable (added 2026-07-25).** Priority 1 rejects a hand-maintained capability matrix because it reproduces the prose-drift problem one layer up; the limitations register has the identical structure and identical exposure, and A.0004 promises a *generated* model card that can only be as current as its source. Each limitation therefore gains a stable ID, a status (`open` / `mitigated` / `retired`), the PRD that changes it, and the evidence artifact demonstrating the change. Retiring a limitation becomes an acceptance criterion of the PRD that claims it — M.0003 retires the internal-migration and origin–destination items, E.0006 retires preclassified entrants, Track B retires the fertility-interpretation item if its gate passes — rather than a documentation task nobody owns. A model card generated from a stale register is worse than stale prose, because it carries the authority of an artifact.

> [!warning] Review comment (2026-07-25) — Apply priority 1's own argument to the limitations register
> Priority 1 rejects a hand-maintained capability matrix because "a hand-maintained matrix is the prose-drift problem it replaces, one layer up", and requires generation from the conformance suite. `scientific-limitations.md` has the identical structure and the identical exposure: it is hand-maintained prose whose numbered items this roadmap cites as load-bearing references (§2 births, §3 migration, §5 entrants, §6–§8 approximations, §13 households, §14 observations), and A.0004 promises a *generated* model card whose limitations section can only come from it. As Tracks E, M, B, R, and H land, each retires or amends specific items — §3 and §4 are closed by M.0003, §5 by E.0006, §2 by Track B if its gate passes — and nothing keeps the register in step. Sembla will then generate a model card from a stale source and publish it as an artifact, which is a worse failure than stale prose because it looks authoritative.
>
> Recommend giving each limitation a stable ID, a status (`open` / `mitigated` / `retired`), the PRD that changes it, and the evidence artifact that demonstrates the change, in a machine-readable form A.0004 generates from. Retiring a limitation then becomes an acceptance criterion of the PRD that claims it, rather than a documentation task nobody owns. This is cheap now — the register is one file with fifteen numbered items — and it is the same discipline the roadmap already demands of the capability matrix.
>
> **Resolution (2026-07-25):** adopted — the register gains stable IDs and status fields, and retiring a limitation becomes an acceptance criterion of the PRD that claims it. A.0004 generates from it.
