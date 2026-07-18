<!-- apitts-audio:start -->
Generated text-to-speech audio (18/07/2026, 1:32:41 pm):

- Whole note
![[_Audio/Scratch/sembla-assessment/001-whole-note.mp3]]

<!-- apitts-audio:end -->

# Sembla — Assessment

**What it is:** a compositional simulation framework with a Lean 4 frontend (DSL, IR exporter, ProofWidgets) and a deterministic Rust runtime, joined by a versioned JSON IR. The thesis: *seed + IR + θ + determinism level ⇒ reproducible results*. Current state: ~7k lines of Rust, ~5k lines of Lean, 74 tests passing, v0.1 success criteria (`DESIGN.md:583`) appear substantially met, with a GPU precision spike just concluded.

## Merits

1. **A genuinely differentiated thesis.** Determinism is treated as a contract, not a best-effort property: Philox-by-coordinate RNG, three explicit determinism levels (audit / portable / fast, `DESIGN.md` §5.2), common random numbers for counterfactual policy comparison (§5.3), and run manifests (§5.4). The CRN "compare" workflow is rare and genuinely valuable for policy work — most agent-based frameworks can't do variance-reduced counterfactuals at all.

2. **Sound architecture.** The IR-as-narrow-waist design (frontend-agnostic backend, Lean as ground-truth semantics, CPU interpreter as executable oracle with differential GPU testing) is the right shape. Conflict resolution via argmin-over-racing-clocks with declared contested resources (§5.1) avoids last-writer-wins without giving up parallelism — a better answer than most ABM frameworks have.

3. **Exceptional engineering discipline for its age.** PRD-driven development (19 PRDs with review artifacts in `.piprd/reviews/`), an adversarially reviewed design doc, an ADR with an explicit selection rule (`docs/decisions/0001-gpu-precision.md`), a dependency allowlist enforced in `scripts/check.sh` (runtime has exactly one external dep: `sha2`), zero TODOs/stubs in the tree.

4. **Intellectual honesty as process.** `spikes/precision/RESULTS.md` is generated atomically, leaves unavailable cells "explicitly unanswered," and refuses to fabricate numbers ("No throughput or accuracy number was fabricated"). The comparison doc (`docs/sembla-vs-pfclbs.md`) concedes the sibling project is more mature. The ADR is marked "measured; decision pending" rather than prematurely closed. This culture is the strongest predictor the project's claims can be trusted later.

5. **De-risking by spike before commitment.** GPU throughput and precision were benchmarked on real hardware (including rented H100s via Terraform) *before* the v0.2 numeric contract was written.

## Key risks (ranked)

1. **The proof gap.** The headline Lean claim — denotational ground truth, compiler rewrites as certified equivalences (§7) — is currently "deferred proofs, specified now." Today the Lean layer is a DSL + exporter + widgets, not a verified compiler (the project's own comparison doc says this plainly). If Sembla is ever judged against its thesis rather than its current state, this gap is where credibility breaks. The proofs are also the hardest part to schedule.

2. **The GPU numeric contract is unresolved.** ADR 0001's decision gate requires full-rate NVIDIA confirmation; the dev machine (M2 Pro/Metal) can't run native f64 or CUDA, so the decision depends on rented cloud GPU infra — and the commit history ("Harden Hyperstack SSH recovery", "Fix H100 CUDA benchmark compatibility") suggests that path has been flaky. v0.2's entire performance story rides on a decision that isn't made yet.

3. **Semantic-accuracy risks are mitigated, not solved.** Synchronous ticks create a one-event-per-resource-per-tick tau-leap bias; the mitigation is a saturation warning counter, and automatic Δt bias detection plus conflict-scope declaration syntax are both listed as open questions (§10.1–2). Wrong-but-reproducible results are still wrong.

4. **The adoption-critical 50% is unbuilt.** Synthetic population initialization — acknowledged in §10.5 as historically more than half the effort in policy microsimulation — is entirely unaddressed, and the calibration/posterior workflow's home is undecided (§10.4). These, not the kernel, will determine whether the apparent target users (policy modelers) can actually use it.

5. **Adoption funnel is narrow at the front door.** "Author your model in Lean 4" excludes nearly all working modelers. The backend being frontend-agnostic mitigates this architecturally, but there is no second frontend, and the Lean toolchain is itself a maintenance surface.

6. **Project-management realities.** No LICENSE (unusable by others as-is), no CI (no `.github/workflows`) — notable for a project whose core promise is *cross-machine bitwise reproducibility*, which is precisely what continuous cross-hardware verification would enforce (Level B remains "unproven"). Single author, ~1 week old, and effort split with the sibling PFCLBS/SKS repo, which is broader and more mature — a fragmentation risk whichever way the relationship goes.

## Biggest de-risking moves

- Land one real proof (the group-by lumping rewrite, §7 target #1) to convert "certified" from aspiration to precedent.
- Close ADR 0001 with the H100 full-rate run.
- Add CI running the determinism test matrix, and add a license.
- Prototype a non-Lean authoring path (even a thin Python/JSON emitter) to test the frontend-agnostic claim.

**Bottom line:** unusually well-reasoned and honestly evidenced for a week-old project; the architecture deserves to survive contact with reality. The risks are concentrated exactly where the ambition is — the unproven proof layer, the unmade GPU precision decision, and the unbuilt population/calibration tooling that real users need.
