# PRD GPU-0001: Collect qualifying CUDA conformance evidence

max_review_cycles: 1

## Dependencies

Read the binding [GPU track README](README.md). Every mandatory precondition is
true. The software/governance queue is accepted, committed and pushed; its exact
corpus and paid-run wrapper are frozen. The operator has approved the exact
saved plan by SHA-256 and price ceiling outside this managed run.

## Context

This is evidence collection, not implementation. It is the only PRD permitted
to apply the already approved plan. It may not modify source, corpus, thresholds
or infrastructure. One paid execution is allowed; review/revision may perform
no second apply.

## Goal

Use the machine-checked approved-plan wrapper to create exactly one qualifying
full-rate NVIDIA VM, run two complete CPU/CUDA corpus repetitions, retrieve and
verify immutable evidence, and prove mandatory teardown/provider reconciliation.

## Requirements

### 1. Fail-closed preflight

Before any apply, run the software-track approval wrapper in check-only mode. It
must verify all GPU README preconditions, including:

- the externally signed approval request verifies only as principal
  `sembla-paid-plan-operator` and namespace
  `sembla-hyperstack-paid-plan/v1` against the tracked fixed signer policy and
  bound policy SHA, is unexpired, and its exact plan/summary SHA values equal the
  saved bytes;
- reviewed repository commit equals clean local HEAD and exists at the configured
  remote ref;
- live plan summary still names exactly one VM and one `/32` SSH rule;
- approved/configured hourly price, absolute deadline and maximum-total-cost
  formula agree;
- device/image/region/profile match the approval;
- disposable session/host/deploy identities match the saved plan and are not
  expired;
- no Terraform state resource, provider VM, provider SSH rule or scoped orphan
  currently exists;
- both live and logged-in-user LaunchAgent watchdog paths can arm and
  acknowledge readiness for the signed absolute deadline/plan SHA, and the
  operator accepts the documented reboot-before-login limitation;
- no execution marker exists for the approved plan hash; and
- `KEEP_VM`, fallback and every unrelated `BENCH_*` selector are absent.

A failed preflight returns before apply and creates no resource. Do not repair,
regenerate or ask the operator inside this run.

### 2. One approved apply and bounded collection

Invoke the wrapper's execute mode exactly once. It must:

1. exclusively claim and fsync the permanent
   `.paid-plan-consumed/<plan-sha256>/record.json` authorization as specified by
   the software PRD;
2. arm and receive readiness acknowledgement from both signed
   absolute-deadline watchdog paths in pre-apply mode;
3. apply the saved plan, never a recomputed or previously consumed plan;
4. reconcile immediately after any failed/interrupted apply so absent state
   cannot hide a billing VM or SSH-rule orphan;
5. wait for the pre-seeded host identity and verified bootstrap;
6. execute only `BENCH_BACKEND_CONFORMANCE=1` using the exact commit/corpus;
7. retrieve complete or checksummed partial evidence;
8. verify remote and local SHA-256 manifests; and
9. run bounded Terraform destroy plus VM/rule report-delete-report provider
   reconciliation on success, mismatch, failure, timeout, TERM and INT.

`KEEP_VM=1` is forbidden. Guest poweroff is never accepted as teardown.

### 3. Qualifying result

Evidence must show:

- a real CUDA backend and full-rate NVIDIA device, never stub/fallback;
- device UUID/model/compute capability, driver, CUDA runtime and NVRTC versions;
- exact clean commit, release binary and corpus hashes;
- two complete repetitions of every case with zero skip/unavailable status;
- exact per-tick state, final state, ordinary/grouped observation, summary,
  exported-state and output-tree agreement with CPU;
- stable plan/rule/mailbox/RNG identities;
- successful deliberate perturbation rejection; and
- canonical overall `passed` report.

Any scientific mismatch is a failed gate. Retain partial evidence but do not
change source or run a rescue matrix.

### 4. Evidence publication

Publish one new immutable tuple directory:

```text
docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid>/
```

containing the canonical report, corpus/command/input manifests, raw logs,
device/software provenance, signed approval request/signature/public signer
identity and safe plan summary/hash, negative control,
benchmark/teardown statuses, Terraform final state, provider reconciliation and
the raw-evidence-only `EVIDENCE-SHA256SUMS` required by the frozen closure
schema.

The evidence README states cost/duration, exact status and residual risks. It
must not include credentials, private keys, user-data, Terraform plan/state or
absolute private paths.

### 5. Final billing proof

Before this implementation returns, require:

- no VM or security-rule address in Terraform state;
- provider reconciliation independently reports zero scoped VMs, zero scoped
  SSH rules and zero orphans of either kind;
- the disposable Tailscale/GitHub/session credentials are revoked/cleaned where
  the existing workflow supports it; and
- evidence manifests verify after cleanup.

A teardown/reconciliation failure is an emergency nonzero result and can never
be revised into approval by editing documentation.

## Allowed files

- `docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid>/**`
  (one new tuple directory only)

No source, PRD, configuration, corpus, fixture, architecture, decision or
infrastructure file may change.

## Non-goals

- No source fix, threshold change, corpus change, optimization or second paid
  run.
- No plan generation, plan substitution, price/device choice or user prompt.
- No trust-on-first-use, broad ingress, credential persistence or teardown
  bypass.
- No acceptance of partial/unavailable/fallback/mismatching evidence.
- All scripts, infrastructure, signer policy, corpus and configuration are
  read-only accepted inputs in this PRD.

## Test guidance

The paid wrapper runs the corpus and teardown checks. After collection, run only
non-network verification:

```bash
python3 -B scripts/run-backend-conformance.py --validate \
  docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid>/cuda-report.json
python3 -B scripts/check-backend-conformance.py
(cd docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid> \
  && sha256sum -c EVIDENCE-SHA256SUMS)
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

Review/revision must not rerun apply. If evidence is not qualifying, stop the
managed run and retain the failed/partial artifact outside acceptance.

## Acceptance criteria

1. A valid unexpired external signature under the fixed tracked
   principal/namespace/policy hash binds every plan/session/commit/corpus/device/
   price/deadline field; an exclusive durable claim prevents concurrent/restarted
   reuse of the exact plan, both
   pre-apply watchdogs acknowledge readiness and apply can occur once only across
   every process/review/revision.
2. Every corpus case completes twice on real CUDA with zero fallback, skip,
   unavailable or CPU/CUDA mismatch.
3. The negative control fails comparison and the canonical report validates
   offline against exact corpus/input/binary/commit/device hashes.
4. One credential-free immutable evidence directory contains all required raw,
   provenance, approval, verdict and checksum files.
5. Benchmark and teardown statuses are distinct, and final state plus
   independent VM/SSH-rule provider reconciliation prove no paid resource or
   orphan remains.
6. No source/configuration/fixture/threshold file changes and no second paid run
   occurs.
7. Offline evidence, full repository, allowlist and diff checks pass.
