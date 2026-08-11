# Direct spike protocol: dense transition-family dispatch

This is a **spike protocol, not a PRD**. Do not pass it to `/piprd run`.
`DECISIONS.md` §M1 requires this direct same-result measurement before any
production transition-family PRD is drafted.

Run it only when
`docs/evidence/cuda-transition-families/profile-gate.json` verifies and lists
`dense_family_dispatch` in `authorized_spikes`.

## Question

Can structurally compatible, contiguous transitions share generated CUDA
functions/launch fragments while retaining the existing dense candidate/claim
layout and every observable semantic, and does that directly improve more than
one model shape?

## Minimal direct arm

Build the smallest committed, runnable arm that answers the question. It is not
a finished optimizer.

1. Classify compatible transition skeletons structurally from validated model
   data. Do not inspect model/transition names or benchmark paths.
2. Keep the executable schedule as maximal contiguous compatible fragments plus
   singleton legacy occurrences. Noncontiguous members may reuse a generated
   function but execute in separate fragments at their original positions.
3. Keep `candidate_offsets[rule_id] + row`, dense candidate/claim arena sizes,
   Philox coordinates, validation separation, conflict/effect/report ordering,
   diagnostics, and outputs unchanged.
4. Generate one hand-written/direct family arm for the profile-authorized phase
   set and retain the existing path as the reference in the same binary.
5. Keep unsupported transitions explicit and legacy; do not make one rejected
   member disable unrelated contiguous fragments.
6. Keep the arm hidden and default-off. It may live behind a spike-only
environment seam or focused example/test harness, but it must be committed,
runnable, and removable without public compatibility cost.

Dense candidate/claim bytes must remain identical. Measure descriptor, generated
source/PTX, module/function, NVRTC, and other incremental bytes rather than
calling capacity unchanged.

## Exactness screen

Before timing, compare direct-arm versus legacy CUDA without normalization on:

- the Australian 2010 hundredth 12-tick grouped workload;
- the profile-authorized second family-reuse shape;
- moderate-, dense-, and few-transition controls;
- mixed eligible/legacy and zero-row fixtures; and
- guard, hazard, claim, effect, reference, checked-integer, and double-write
  failures.

Pin logical candidate ID, `rule_id`, `rule_word`, entity row, Philox tuple,
minimum status/payload/message, conflict winner, effect ownership,
fired/deferred ordering, state/hash, grouped sidecars, and all scientific bytes.
CPU-versus-CUDA uses the parent README's comparator contract; candidate versus
legacy uses no normalization. A deliberate one-byte perturbation must fail.

A semantic mismatch is a useful negative result: record it exactly, leave the
arm disabled, and close dense dispatch. Do not time an invalid arm.

## Direct timing screen

Use one locked release binary and adjacent arms, one warm-up, and at least three
timed repetitions. Record whole wall, simulation window, setup/NVRTC, launch and
function counts, generated/PTX size, kernel/API summaries, VRAM/RSS, worker/stream
mode, output comparison, and raw checksums.

A positive result requires:

- at least 1.25x Australian single-worker simulation throughput;
- at least 1.15x Australian whole-run wall improvement;
- at least 1.10x simulation throughput on a second structurally distinct
  family-reuse shape;
- no greater than 5% whole-run regression on moderate-, dense-, or
  few-transition controls; and
- exactness on every valid/failing case.

Write raw evidence and a machine-recomputed result under
`docs/evidence/cuda-transition-families/dense-dispatch-spike/`. The result is
`positive`, `negative`, or `inconclusive`; all are valid spike outcomes. Append a
measured `DECISIONS.md` note.

Only a positive, exact result authorizes drafting production metadata and dense
family-dispatch PRDs. It does not authorize candidate compaction unless the
profile independently authorized and a separate compaction spike directly wins.
