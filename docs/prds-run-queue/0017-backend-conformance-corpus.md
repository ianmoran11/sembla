# PRD 0006: Freeze the CPU/CUDA backend conformance corpus

max_review_cycles: 3

## Dependencies

Contract-governance PRDs 0001–0005 are accepted. Read the binding [contract-governance README](../prds-contract-governance/README.md). CPU
remains the semantic oracle; this PRD performs no cloud or paid-device action.

## Context

CPU/CUDA equivalence is already tested through focused tests and
`diff-backends`, but there is no one versioned corpus/report that states which
semantic families constitute a qualifying backend. Compilation and scattered
tests are not retained conformance evidence.

## Goal

Add a fixed, CPU-safe corpus, canonical report schema and offline-tested runner;
prepare an opt-in Hyperstack stage that can execute the same corpus later
without changing it during the paid session.

## Requirements

### 1. Corpus manifest

Add `fixtures/backend-conformance/corpus.json` with schema
`sembla.backend-conformance-corpus/v1`. Its top-level ordered
`semantic_families` array is the authoritative family universe. Every family has
a stable slug ID, description, required output categories and minimum independent
case count. The array must equal this table exactly and in this order:

| Family ID | Required report categories | Minimum independent cases |
| --- | --- | ---: |
| `parameter_evaluation` | `per_tick_state`, `final_state`, `results` | 2 |
| `expression_evaluation` | `per_tick_state`, `final_state`, `results` | 2 |
| `philox_coordinates` | `per_tick_state`, `final_state`, `identity_records` | 2 |
| `racing_clock_selection` | `per_tick_state`, `final_state`, `results` | 2 |
| `resource_conflict_resolution` | `per_tick_state`, `final_state`, `results` | 1 |
| `effect_application` | `per_tick_state`, `final_state` | 2 |
| `input_mailbox_delivery` | `per_tick_state`, `final_state`, `results`, `identity_records` | 1 |
| `ordinary_observation` | `results`, `ordinary_observations` | 1 |
| `grouped_observation` | `results`, `grouped_observations` | 1 |
| `summary_reduction` | `results`, `summaries` | 1 |
| `state_artifact_roundtrip` | `final_state`, `exported_state` | 1 |
| `stable_identity` | `identity_records`, `final_state` | 2 |
| `feature_gating` | `grouped_observations` | 1 |
| `empty_boundary_behavior` | `results`, `ordinary_observations`, `summaries` | 1 |

Every case references only declared families; every family has the required
number of distinct non-alias case IDs; unknown, uncovered or duplicate family
IDs fail. The implementation may add no family or lower a category/count without
a later decision/PRD revision.

Each naturally ordered case contains:

- stable case ID and purpose;
- existing or new model/plan path and canonical hash;
- population/state input and hash;
- optional parameter artifact and hash;
- seed, tick count and enabled features;
- expected semantic families exercised;
- the exact output categories that must compare; and
- `expected_path` plus SHA-256 for a strict semantic-hash golden under
  `fixtures/backend-conformance/expected/<case-id>.json`.

The corpus contains at least these independently useful cases:

1. legacy SIR model with default parameters and zero/nonzero hazards;
2. direct-stable SIR plan with a `sembla.parameters/v1` artifact;
3. ordinary observations and summaries, including empty/zero outputs;
4. grouped observations under the explicit feature flag;
5. contested resource claims, accepted/deferred effects and tie-breaking;
6. linked two-box input/mailbox behavior and stable plan identities;
7. state-artifact initialization/export/reload over two chained segments; and
8. integer/real expression boundary cases, checked arithmetic and deterministic
   Philox coordinates.

Add small dedicated model/state fixtures under the corpus directory only where
existing fixtures cannot isolate a required family. No accepted existing fixture
is regenerated. Every claimed family maps to at least one case; every case is
non-redundant and bounded for routine CPU execution.

Generate each expected semantic-hash golden with the CPU oracle binary built
`--locked` in a detached `track_base` worktree. New case inputs may be copied
read-only into that worktree because existing model/plan/state schemas are
frozen. For the new parameter-envelope case, run the equivalent legacy values
through the old binary and require the current envelope path to match those
semantic hashes. Each golden contains exactly the case ID and required hash
categories, compact canonical JSON/no newline; the corpus binds its exact bytes.
Current CPU repetitions must match each other **and** the bound old-oracle golden,
so deterministic semantic drift cannot bless itself.

### 2. Canonical report

Define `sembla.backend-conformance/v1` with this exact top-level shape and no
additional fields:

```json
{
  "schema_version": "sembla.backend-conformance/v1",
  "corpus": {"schema_version": "sembla.backend-conformance-corpus/v1", "sha256": "..."},
  "source": {
    "repository_commit": "40 lowercase hex",
    "checkout_clean": true,
    "source_tree_sha256": "64 lowercase hex",
    "release_binary_sha256": "64 lowercase hex"
  },
  "runner_versions": {"name": "version"},
  "backend": {
    "kind": "cpu_oracle|cuda",
    "availability": "available|unavailable",
    "unavailable_reason": null,
    "identity": {"name": "...", "uuid": "...", "driver": "...", "runtime": "...", "nvrtc": "..."}
  },
  "repetitions": [
    {
      "ordinal": 0,
      "cases": [
        {
          "id": "case_slug",
          "families": ["family_slug"],
          "inputs": [{"role": "model", "path": "...", "sha256": "..."}],
          "command": ["sembla", "..."],
          "seed": 1,
          "ticks": 2,
          "features": [],
          "hashes": {
            "per_tick_state": ["..."],
            "final_state": "...",
            "results": "...",
            "summaries": "...",
            "ordinary_observations": "...",
            "grouped_observations": "...",
            "exported_state": "...",
            "identity_records": "..."
          },
          "status": "passed|failed|not_run"
        }
      ]
    }
  ],
  "family_verdicts": {"family_slug": "passed|failed|unavailable"},
  "negative_control": {"mutation": "one_byte", "rejected": true},
  "overall_status": "passed|unavailable|failed",
  "qualifying": false
}
```

Object keys shown are required except individual hash values whose corpus case
explicitly declares the category inapplicable; such values are JSON `null`, not
omitted or empty strings. `negative_control.rejected` is Boolean or `null` under
the matrix below. `backend.identity` uses the same keys with empty strings for
CPU/unavailable fields and strict non-empty CUDA fields. Cases/families always
follow corpus order. Strict validation rejects unknown fields and every enum
combination outside this exhaustive matrix:

| Mode/outcome | Availability/reason | Repetitions/cases | Family verdicts | Negative control | Overall / qualifying |
| --- | --- | --- | --- | --- | --- |
| CPU completed | `available` / `null` | ordinals `[0,1]`; every case present as `passed` or `failed` | only `passed|failed` | `true|false` | `passed` iff every case/family passes and control is true; qualifying always false |
| CUDA unavailable before execution | `unavailable` / non-empty reason | empty array | every family `unavailable` | `null` | `unavailable` / false |
| CUDA available and completed | `available` / `null` | ordinals `[0,1]`; every case present | only `passed|failed` | `true|false` | `passed` iff every case/family passes and control is true; qualifying true only with clean source |
| Available backend abort/failure | `available` / `null` | ordinals `[0,1]`; every corpus case present, unfinished cases `not_run` with all hashes null | affected families `failed`; no `unavailable` | false or null if not reached | `failed` / false |

`--cuda` on a dirty tree refuses before backend execution and writes no report;
it is neither unavailable nor qualifying. A `passed` report contains no
`failed`, `not_run`, `unavailable`, null required hash or false/null control.

`source_tree_sha256` is SHA-256 over domain
`sembla.backend-conformance-source-tree/v1`, a NUL byte, then bytewise-path-sorted
records for exactly `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
`crates/**`, `fixtures/backend-conformance/**`,
`scripts/run-backend-conformance.py` and
`scripts/check-backend-conformance.py`. Each record is exactly `path_utf8 NUL token NUL digest_hex LF`, where paths are
repository-relative POSIX UTF-8 with no NUL and tokens are only
`file:100644`, `file:100755`, `symlink:120000` or `deleted:000000`. Regular-file
content is exact bytes; symlink content is the UTF-8 link target bytes; deleted
content is empty bytes. `digest_hex` is lowercase SHA-256 of those content bytes
(`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
for deleted). Include non-ignored untracked files inside those roots;
exclude ignored build/secrets and everything outside the roots. A tracked path
missing from the worktree emits `deleted:000000`; mode changes alter the token.
The checkout-clean flag is derived from Git independently. Tests freeze exact
payload bytes, path ordering, tokens, symlinks, additions, modifications and
deletions.

No wall-clock timestamp or timing enters identity-bearing report bytes. A whole
runner invocation contains both repetitions; repeating the invocation against
the same source/backend produces byte-identical report bytes. Canonical JSON is
compact, recursively key-sorted and has no trailing newline.

`unavailable` is valid only for an explicitly requested CUDA mode on a machine
without the feature/device; it is never a passing or qualifying result.

### 3. Runner and CLI output

Add `scripts/run-backend-conformance.py` with modes:

- `--cpu`: run every case twice through the CPU oracle, require internal
  repetition equality and exact match to the corpus-bound `track_base` CPU
  semantic-hash goldens; a development tree may be
  dirty only when its deterministic source-tree digest is recorded and the
  report is explicitly non-qualifying;
- `--cuda`: require a clean checkout, the real CUDA backend and CPU comparison
  for every case;
- `--validate <report>`: validate schema, corpus/input hashes, completeness and
  verdict offline without executing a backend.

The runner may invoke the CLI binary but imports no Rust library. It must fail on
unknown/duplicate/skipped/reordered cases, output-set drift, fallback, a
scientifically meaningful mismatch or a comparator that accepts a deliberate
one-byte perturbation.

Add the explicit optional `diff-backends --report-json <path>` output required
by the corpus runner. It writes the case-level machine comparison atomically;
without the flag, existing human output/default behavior and tests remain
byte-compatible. Comparisons include exact per-tick/final state and all output
bytes; only backend identity fields explicitly excluded by the existing
contract may be normalized.

### 4. CPU-safe enforcement

Add focused tests for corpus shape, deterministic report bytes, missing/corrupt
cases, feature-off unavailability and negative controls. Integrate only the
bounded CPU/offline checks into normal repository gates; ordinary CI never
requires CUDA hardware.

Register all eight new identifiers—`sembla.backend-conformance-corpus/v1`,
`sembla.backend-conformance/v1`,
`sembla.backend-conformance-source-tree/v1`,
`sembla.backend-conformance-logs/v1`,
`sembla.hyperstack-plan-consumption/v1`,
`sembla.hyperstack-watchdog-readiness/v1`,
`sembla.hyperstack-paid-approval-request/v1` and
`sembla.backend-conformance-closure/v1`—with owners/readers/writers/evidence,
map them to the existing registry/conformance concept in the canvas projection,
and regenerate `generated-contract-index.md`; stale generated output is a hard
failure. Update architecture/runtime fitness notes. Do not claim qualifying CUDA
evidence until the second-stage GPU PRDs.

### 5. Prepared Hyperstack stage

Extend the existing Hyperstack benchmark driver with one mutually exclusive
opt-in selector:

```bash
BENCH_BACKEND_CONFORMANCE=1 bash run-demographic-benchmark.sh
```

The stage uses the exact committed corpus/release binary, runs two complete CUDA
repetitions, retains raw logs/report/negative control, packages checksummed
partial evidence on failure, rejects `KEEP_VM=1` and incompatible `BENCH_*`
selectors, and enters the existing bounded destroy/reconciliation path on every
exit.

### 6. Externally signed paid-plan approval

Extend `review-paid-plan.py` with a credential-free `--approval-request` mode
that emits this exact no-unknown-field JSON object:

```json
{
  "schema": "sembla.hyperstack-paid-approval-request/v1",
  "plan": {
    "sha256": "<64 lowercase hex>",
    "safe_summary_sha256": "<64 lowercase hex>"
  },
  "source": {
    "repository_commit": "<40 lowercase hex>",
    "source_tree_sha256": "<64 lowercase hex>",
    "corpus_sha256": "<64 lowercase hex>",
    "release_binary_sha256": "<64 lowercase hex>",
    "release_build_recipe": "cargo build --locked --release --features cuda",
    "signer_policy_sha256": "<64 lowercase hex>"
  },
  "target": {
    "region": "<ascii>",
    "environment": "<ascii>",
    "flavor": "<ascii>",
    "device": "<ascii>",
    "image": "<ascii>",
    "ssh_cidr": "<canonical IPv4/32>"
  },
  "session": {
    "console_id": "<ascii>",
    "console_host_key_sha256": "<64 lowercase hex>",
    "vm_host_key_sha256": "<64 lowercase hex>",
    "deploy_public_key_sha256": "<64 lowercase hex>",
    "tailscale_tailnet_id": "<ascii>",
    "tailscale_auth_fingerprint_sha256": "<64 lowercase hex>",
    "tailscale_mint_epoch_seconds": 0,
    "tailscale_expiry_epoch_seconds": 0
  },
  "pricing": {
    "currency": "USD",
    "hourly_nano_usd": 0,
    "public_ip_fixed_nano_usd": 0,
    "deadline_duration_seconds": 0,
    "maximum_total_nano_usd": 0
  },
  "created_epoch_seconds": 0,
  "approval_expires_epoch_seconds": 0,
  "destroy_deadline_epoch_seconds": 0
}
```

Every placeholder integer is a non-negative JSON integer within signed 64-bit
range; no float/exponent/decimal JSON token is allowed. All strings are
non-empty printable ASCII without leading/trailing whitespace. Epochs are UTC
Unix seconds. Require mint ≤ created < approval expiry ≤ destroy deadline <
Tailscale expiry and at least 900 seconds of Tailscale cleanup margin after the
destroy deadline. `deadline_duration_seconds` equals deadline minus created.
Parse the discovered provider USD decimal losslessly to nanodollars (at most
nine fractional digits; reject other currencies/precision), then require
`maximum_total_nano_usd = ceil(hourly_nano_usd × deadline_duration_seconds /
3600) + public_ip_fixed_nano_usd`, using checked integer arithmetic.

Canonical signed bytes are UTF-8 compact JSON, recursively lexicographically
sorted by Unicode code point, arrays in source order, separators exactly `,` and
`:`, no insignificant whitespace and no trailing newline. Because accepted
strings are printable ASCII and numbers are integers, no cross-language float or
Unicode-normalization choice remains. Request identity is SHA-256 of these exact
bytes; it is reported externally rather than embedded circularly.

Preparation must happen **before** plan generation because the disposable
session values are embedded in user-data. Extend session metadata to retain the
non-secret Tailscale mint/expiry data needed above.

The signer policy is fixed at
`spikes/precision/infra-hyperstack/paid-plan-approvers` with exactly one allowed
principal, `sembla-paid-plan-operator`, and namespace
`sembla-hyperstack-paid-plan/v1`. Its format is exactly one LF-terminated OpenSSH
allowed-signers line, no comments/blanks: the principal, one ASCII space,
`namespaces="sembla-hyperstack-paid-plan/v1"`, one space, key type
`ssh-ed25519`, one space and one base64 public-key blob; no trailing comment,
wildcard, certificate or additional principal/key is accepted. Add that path to
the exact-file `.gitignore` allowlist but do not create a production key in this
PRD. Between the software
and GPU runs, the operator commits/pushes the public-key policy in a separately
reviewed commit; the signing private key remains on a separate device and is
unavailable to the Pi host, SSH agent, Keychain and managed environment. The
approval request binds the tracked policy file's SHA-256.

The operator signs the request outside the Pi/managed-run environment with an
OpenSSH detached signature under that fixed principal/namespace. The wrapper
always loads the tracked fixed path and has no environment/CLI override for the
principal, namespace or policy file. Signature expiry, policy hash and every
bound value are verified before apply. A caller-supplied file/hash or a newly
generated key without the committed signer root is never approval. Offline tests
use fixture keys only under temporary test directories. Normal software CI may
report `hardware_signer_not_prepared` while the production policy file is absent;
it validates the verifier with fixtures and does not treat absence as paid
approval or attempt provisioning.

### 7. One-shot approved-plan wrapper and billing controls

Add `spikes/precision/infra-hyperstack/run-approved-backend-conformance.sh` to
the module's exact-file `.gitignore` allowlist, with `--check` and `--execute`
modes. It is the only command the GPU evidence PRD may
use. It verifies the signed request, clean/pushed commit, session expiry, exact
plan/summary/device/price/corpus bindings, empty Terraform/provider inventory and
watchdog readiness without prompting. It never creates, recomputes or approves a
plan.

Before spawning `terraform apply`, execute mode claims the exact ignored path
`.paid-plan-consumed/<plan-sha256>/` with same-filesystem atomic `mkdir` (mode
0700); existence is permanent consumption and concurrent claim failure. Inside
the winning directory it writes `record.json` mode 0600 via O_EXCL temporary
file, fsyncs file, atomically renames, and fsyncs the directory before apply.
Every later/concurrent execution—including restart or review/revision after
successful teardown—must reject that consumed SHA. A crash after claim but
before apply consumes the authorization without spending; it is never retried.
`record.json` is strict compact canonical
`sembla.hyperstack-plan-consumption/v1` with exactly `schema`, `plan_sha256`,
`approval_request_sha256` and integer `claimed_epoch_seconds`; it is evidence,
not a reusable token. Automated cleanup never removes it; only post-run operator
housekeeping after the managed run may archive/delete it.

Extend `destroy-deadline.sh` with a pre-apply absolute-deadline mode backed by a
per-session macOS LaunchAgent (`RunAtLoad` plus calendar/deadline handling) and
the existing live watchdog. It stores no provider secret, calls the Keychain
injection helper at execution, and survives managed-process failure plus
sleep/wake while the prepared macOS user session remains logged in. After a host
reboot it can run only after that user logs in and the login Keychain is
available; the PRD must not claim an unconditional boot-time control. When it
runs, it always performs bounded Terraform destroy followed by provider
reconciliation even when Terraform state is empty. Arm must complete a strict
compact canonical `sembla.hyperstack-watchdog-readiness/v1` acknowledgement with
exactly `schema`, `plan_sha256`, integer `destroy_deadline_epoch_seconds`,
`launchd_label`, integer `live_pid`, `launchd_state: "ready"`,
`live_state: "ready"`, `requires_logged_in_user: true` and integer
`acknowledged_epoch_seconds`; `launchd_label` is exactly
`com.sembla.hyperstack.destroy.<first-16-plan-sha256-hex>`. Write it only after
`launchctl print` confirms that label and the live process is responsive; fsync
before apply. A background PID alone is not ready.

The wrapper recomputes and verifies the exact integer-nanodollar cost equation
above against the discovered provider price and signed fields. Documentation must state honestly that this is the
operator-approved exposure ceiling and dual recovery control, not a provider
billing guarantee during simultaneous host/network/provider failure; account
alerts and immediate provider escalation remain residual controls.

Extend `reconcile-orphans.sh` to inventory/re-query/delete both scoped VMs and
scoped SSH security rules, including state-missing VM-only and rule-only cases.
Final success requires empty Terraform state and empty provider inventories for
both resource kinds. Update the infrastructure README/RUNBOOK so this
conformance stage uses only the signed wrapper, signer preparation,
watchdog-before-apply and wrapper-owned apply; remove/replace any direct-apply or
watchdog-after-apply instruction for this stage.

Offline tests use fake Terraform/provider/OpenSSH commands and prove:

- unsigned, self-supplied, wrong-principal/policy, expired, non-canonical,
  unknown-field, float/overflow/cost-math or otherwise mismatched approval cannot
  reach apply;
- preparation precedes plan/request and mint/expiry metadata is bound;
- the exclusive durable claim is fsynced before apply and survives interruption,
  teardown and process restart;
- two overlapping processes plus later sequential/restarted execution can invoke
  apply at most once; a crash after claim cannot be retried;
- launchd/live watchdog readiness is acknowledged before apply, survives a
  simulated managed-process restart and logged-in sleep/wake, records the
  reboot-before-login limitation, and covers state-missing apply-timeout orphans;
- VM and SSH-rule orphans are independently discovered/deleted/re-queried;
- benchmark failure and teardown status remain distinct; and
- all stage-selector/evidence/cleanup properties above hold.

This PRD validates all paths without credentials and never creates a plan,
resource or network request.

### 8. Prepared hardware-closure generator

The qualifying evidence directory is exactly
`docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid>/`;
the canonical closure path is
`docs/evidence/backend-conformance/hardware-closure.json`. Freeze
`sembla.backend-conformance-closure/v1` as this exact strict JSON shape:

```json
{
  "schema": "sembla.backend-conformance-closure/v1",
  "status": "qualified_exact_revision",
  "source": {
    "repository_commit": "<40 lowercase hex>",
    "source_tree_sha256": "<64 lowercase hex>",
    "corpus_sha256": "<64 lowercase hex>",
    "release_binary_sha256": "<64 lowercase hex>"
  },
  "device": {
    "name": "<ascii>",
    "uuid": "<ascii>",
    "compute_capability": "<ascii>",
    "driver": "<ascii>",
    "runtime": "<ascii>",
    "nvrtc": "<ascii>",
    "region": "<ascii>",
    "flavor": "<ascii>",
    "image": "<ascii>"
  },
  "conformance": {
    "cpu_report_sha256": "<64 lowercase hex>",
    "cuda_report_sha256": "<64 lowercase hex>",
    "negative_control_sha256": "<64 lowercase hex>",
    "repetitions": 2,
    "case_count": 0,
    "families": ["<the exact ordered fourteen-family array>"],
    "cpu_result": "passed",
    "cuda_result": "passed",
    "negative_control_rejected": true
  },
  "authorization": {
    "request_sha256": "<64 lowercase hex>",
    "signature_sha256": "<64 lowercase hex>",
    "consumption_record_sha256": "<64 lowercase hex>",
    "watchdog_readiness_sha256": "<64 lowercase hex>"
  },
  "cleanup": {
    "teardown_report_sha256": "<64 lowercase hex>",
    "reconciliation_report_sha256": "<64 lowercase hex>",
    "terraform_state": "empty",
    "provider_vm_inventory": "empty",
    "provider_ssh_rule_inventory": "empty"
  },
  "evidence": {
    "session_metadata_sha256": "<64 lowercase hex>",
    "logs_tree_sha256": "<64 lowercase hex>",
    "sha256s_sha256": "<64 lowercase hex>"
  },
  "validators": {
    "backend_conformance": "passed",
    "evidence_checksums": "passed",
    "signed_authorization": "passed",
    "one_shot_consumption": "passed",
    "watchdog": "passed",
    "teardown_reconciliation": "passed"
  }
}
```

`case_count` is a positive signed-64-bit JSON integer; every other placeholder
uses the strict lexical/type rules above. The only accepted literal values are
shown, unknown fields are rejected, and families must equal the corpus array.
Use the same compact recursively key-sorted canonical JSON/no-newline encoding
as the conformance report. The path-sorted root
`docs/evidence/backend-conformance/SHA256SUMS` includes every retained regular
file below that root except itself, including `hardware-closure.json`; each line
is lowercase hash, two ASCII spaces, repository-relative POSIX path and LF,
bytewise sorted by path. To avoid a cycle, `evidence.sha256s_sha256` hashes the
qualifying directory's separately generated `EVIDENCE-SHA256SUMS`, which
includes every raw regular evidence file except itself using the same line
format but tuple-directory-relative paths. Symlinks and non-regular files are
rejected in evidence.
`logs_tree_sha256` hashes domain bytes
`sembla.backend-conformance-logs/v1` plus NUL and the same exact path-record
encoding over only non-symlink regular `logs/**` files (all token
`file:100644`), bytewise path sorted.

Add `scripts/write-backend-conformance-closure.py`. Given that exact qualifying
evidence directory and the fixed root closure path, it deterministically writes
`hardware-closure.json` and final root `SHA256SUMS` and refuses corrupt/missing
evidence, stale commit/corpus tuples,
non-pass cases, failed negative control or incomplete VM/rule teardown/
reconciliation. Synthetic fixtures test both qualifying and every refusal
branch. The software track uses it only in tests; no hardware closure is emitted
yet.

Extend `check-backend-conformance.py` now—not during the paid run—so synthetic
fixtures prove its freshness states: a matching closure reports
`qualified_exact_revision`; source/corpus drift reports `evidence_required`
without pretending qualification; corrupted evidence/closure is a hard failure.
Ordinary CPU CI need not fail solely for honest stale status, but release/decision
closure cannot claim current CUDA qualification.

## Allowed files

- `fixtures/backend-conformance/**` (new)
- `scripts/run-backend-conformance.py` (new)
- `scripts/check-backend-conformance.py` (new)
- `scripts/tests/test_run_backend_conformance.py` (new)
- `scripts/write-backend-conformance-closure.py` (new)
- `scripts/tests/test_write_backend_conformance_closure.py` (new)
- `scripts/check.sh`
- `scripts/check-rust.sh` (invocation only if needed)
- `crates/sembla-cli/src/diff_backends.rs`
- `crates/sembla-cli/src/tests.rs`
- `crates/sembla-cli/tests/backend_conformance.rs` (new)
- `crates/sembla-cuda/tests/backend_conformance.rs` (new)
- `.github/workflows/gpu-differential.yml` (documentation/offline-validation
  wiring only; no runner/provisioning)
- `spikes/precision/infra-hyperstack/.gitignore`
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`
- `spikes/precision/infra-hyperstack/run-approved-backend-conformance.sh` (new)
- `spikes/precision/infra-hyperstack/remote-run-spike.sh`
- `spikes/precision/infra-hyperstack/review-paid-plan.py`
- `spikes/precision/infra-hyperstack/prepare-paid-session.sh`
- `spikes/precision/infra-hyperstack/tailscale-auth-key.py`
- `spikes/precision/infra-hyperstack/destroy-deadline.sh`
- `spikes/precision/infra-hyperstack/reconcile-orphans.sh`
- `spikes/precision/infra-hyperstack/README.md`
- `spikes/precision/infra-hyperstack/RUNBOOK.md`
- `scripts/tests/test_run_approved_backend_conformance.py` (new)
- `scripts/tests/test_review_paid_plan.py` (new)
- `scripts/tests/test_destroy_deadline.py` (new)
- `scripts/tests/test_reconcile_orphans.py` (new)
- `scripts/tests/test_tailscale_auth_key.py`
- `scripts/tests/test_keychain_credentials.py`
- `scripts/tests/test_prepare_paid_session.py`
- `docs/architecture/artifact-registry.json`
- `docs/architecture/generated-contract-index.md`
- `docs/architecture/Contracts.canvas`
- `docs/architecture/runtime.md`
- `docs/architecture/fitness-functions.md`

## Non-goals

- No paid plan, apply, device execution, network or credential access.
- No CPU/CUDA semantic, RNG, arithmetic, kernel, performance or backend API
  change beyond the required additive `--report-json` option.
- No backend trait, fallback or normalization of scientific bytes.
- No timing/performance gate and no regeneration of accepted goldens.

## Test guidance

Run focused corpus and Hyperstack offline tests plus:

```bash
python3 -B scripts/run-backend-conformance.py --cpu
python3 -B scripts/check-backend-conformance.py
python3 -m unittest -v \
  scripts/tests/test_run_approved_backend_conformance.py \
  scripts/tests/test_review_paid_plan.py \
  scripts/tests/test_destroy_deadline.py \
  scripts/tests/test_reconcile_orphans.py \
  scripts/tests/test_write_backend_conformance_closure.py
bash scripts/check-rust.sh
bash scripts/check-determinism.sh
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

Run the complete credential-free Terraform/shell/Python validation sequence
from the infrastructure README because allowed infrastructure files change.

## Acceptance criteria

1. The corpus family array equals the frozen fourteen-row table exactly; every
   minimum independent-case/category obligation is met by complete,
   non-redundant, bounded cases whose expected semantic-hash goldens are generated
   by and bound to the `track_base` CPU oracle.
2. The canonical report has the exact strict shape/status matrix above, uses the
   frozen source-tree digest domain/roots, contains two internal repetitions when
   available, and two whole CPU runner invocations produce identical bytes.
3. The runner rejects skipped/unknown/corrupt cases, fallback, output drift and
   the deliberate perturbation.
4. Existing human CLI behavior and scientific outputs remain unchanged; any new
   machine report is additive.
5. Normal CI runs only CPU/offline checks and represents CUDA unavailability as
   non-passing rather than fallback.
6. The Hyperstack selector is non-combinable, content-bound, fail-closed and
   guaranteed to package evidence and attempt mandatory teardown on every exit,
   as proven without provisioning.
7. Only a valid external OpenSSH signature under the fixed tracked signer
   policy over the complete unexpired plan/session/commit/corpus/device/price/
   deadline request can authorize apply; a durable pre-apply marker prevents
   every repeated execution.
8. Live/logged-in-LaunchAgent watchdog readiness and provider reconciliation cover
   state-missing VM/SSH-rule orphans; the signed exposure calculation and its
   simultaneous-failure residual risk are enforced/documented without claiming
   a provider billing guarantee.
9. The prepared closure generator rejects every corrupt, stale, non-passing or
   incompletely torn-down synthetic evidence fixture.
10. New identifiers/projections and the generated index are current, and all
    Rust, determinism, architecture, infrastructure-offline and full repository
    checks pass.
