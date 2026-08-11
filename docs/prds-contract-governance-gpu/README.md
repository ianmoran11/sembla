# Qualifying CUDA conformance evidence PRDs

**Status:** approved as a separately gated second managed run on 2026-08-11.

This folder collects qualifying CUDA evidence for the corpus implemented by the
contract-governance software track. It is intentionally separate from the
unattended software queue because Hyperstack billing policy requires explicit
operator approval of the exact saved plan after the tested commit, device,
stock and live price are known.

Run only after all preconditions below are true. Start Pi inside the prepared
credentialed environment with current working directory at the Sembla repository
root (`pwd -P` equals `git rev-parse --show-toplevel`):

```text
/piprd run docs/prds-contract-governance-gpu/0001-qualifying-cuda-evidence.md
```

`/piprd` requires the first PRD file path, not the folder. Once started from the
prepared environment, this managed run must not ask for further input.

## Mandatory preconditions

The operator—not a PRD agent—must complete these before starting `/piprd`:

1. The software/governance queue is approved, committed and pushed. The exact
   40-hex commit is the repository reference in the paid plan.
2. The Sembla working tree is clean and `.piprd-config.json` is present.
3. The backend-conformance corpus and runner pass their offline validation.
4. Keychain/Tailscale/SSH prerequisites in
   `spikes/precision/infra-hyperstack/README.md` and `RUNBOOK.md` pass.
5. Live discovery confirms the exact qualifying full-rate NVIDIA device, CUDA
   image, stock and account price.
6. Reconciliation reports no existing paid or orphaned resource.
7. Before planning, the disposable Tailscale/session credentials, pinned host
   identity and deploy key are prepared; non-secret IDs/fingerprints plus exact
   mint/expiry times are recorded and remain valid through the approved apply.
8. The repository contains the separately reviewed, committed and pushed fixed
   signer policy at `spikes/precision/infra-hyperstack/paid-plan-approvers`, with
   exactly principal `sembla-paid-plan-operator`; its private key remains on a
   separate device and is unavailable to this Mac, Pi, SSH agent and Keychain.
9. A sensitive saved Terraform plan has then been generated for exactly one VM
   and one `/32` SSH rule. `review-paid-plan.py --approval-request` binds its
   SHA-256, safe summary, commit/corpus, signer-policy hash, session,
   device/image/region, network, hourly price, maximum total cost and absolute
   deadline. The operator signs those canonical bytes externally under namespace
   `sembla-hyperstack-paid-plan/v1`.
10. The Pi process running `/piprd` inherits the prepared credentialed child
    environment plus only the approval request/signature and tracked public
    signer policy. The signing private key is unavailable to that process. All
    commands can execute non-interactively.
11. The Mac is on power, the prepared user session remains logged in, no reboot
    is scheduled, both watchdogs acknowledge readiness, and the signed approval
    explicitly accepts the reboot-before-login residual risk.

General approval of this PRD folder and a caller-supplied hash are not paid-plan
approval. Only the valid unexpired external signature over every bound field is
approval. If any exact precondition is absent, expired or mismatched, stop before
apply. Never regenerate or substitute a plan, key, commit, device, price,
deadline or network identity inside the managed run.

## Authority and safety

- The infrastructure README and RUNBOOK are binding for provisioning, pinned
  host trust, artifact collection, mandatory destroy and orphan reconciliation.
- The conformance corpus/report contract from the software track is binding.
- CPU is the semantic oracle. Every case must run the real CUDA backend with no
  fallback.
- Benchmark failure is valid evidence; teardown failure is an emergency. The
  live and logged-in-user LaunchAgent absolute-deadline watchdogs are armed
  before apply and always attempt bounded destroy plus VM/security-rule provider
  reconciliation, including when Terraform state is empty. The driver repeats
  those checks on success, failure, timeout, TERM and INT.
- `KEEP_VM=1` and every teardown bypass are forbidden.
- The signed cost ceiling and those watchdogs bound the planned session but
  cannot guarantee provider billing stops during host reboot before login or
  simultaneous operator-host, network and provider-control-plane failure. That
  residual risk is part of exact plan approval and requires account alerts/
  provider escalation.

## Qualifying evidence

A qualifying report must bind:

- exact repository commit and clean-checkout result;
- release binary hash and corpus manifest/hash;
- model/plan/state/population inputs and hashes;
- NVIDIA device name, UUID, compute capability and full-rate classification;
- driver, CUDA runtime, NVRTC and relevant library versions;
- command lines, seeds, ticks and enabled features;
- two complete repetitions of every corpus case;
- per-tick state hashes, final-state hashes, ordinary/grouped observations,
  summaries and output-tree comparisons;
- zero unavailable, skipped or fallback cases;
- negative comparator control;
- benchmark and teardown status separately; and
- final empty Terraform state, no provider orphan and verified SHA-256
  manifest.

Evidence from another commit or corpus revision is historical and cannot satisfy
this run.

## Run order

| PRD | Purpose |
| --- | --- |
| 0001 | Apply only the approved plan, collect qualifying conformance evidence and destroy |
| 0002 | Verify, register and document the immutable evidence and close the hardware gate |

## Global non-goals

- No source, corpus, threshold or contract fix during a paid run.
- No provisioning choice or plan generation by a PRD agent.
- No performance optimization or speedup claim.
- No broad SSH rule, trust-on-first-use or credential persistence.
- No acceptance of compilation, CUDA stubs, CPU fallback, skipped cases or
  partial evidence as qualifying conformance.
