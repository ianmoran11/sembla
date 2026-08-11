# PRD GPU-0002: Register evidence and close the real-device gate

max_review_cycles: 3

## Dependencies

GPU PRD 0001 is approved and its paid resource is destroyed. Read the binding
[GPU track README](README.md). This PRD is offline and may not provision or
contact cloud services.

## Context

The qualifying evidence is immutable and bound to one commit/corpus. Maintained
documentation must now distinguish that exact passed evidence from ordinary CI,
future commits and performance claims.

## Goal

Verify the retained artifact independently, publish a hardware closure record,
update architecture/fitness documentation and leave the repository with a
truthful exact-commit conformance status.

## Requirements

### 1. Independent hardware closure record

Create `docs/evidence/backend-conformance/hardware-closure.json` with exactly the
strict `sembla.backend-conformance-closure/v1` shape, canonical encoding, literal
validator results and tuple directory frozen by software PRD 0017. No alternate
field, hand-entered verdict, timestamped directory or schema interpretation is
permitted.

Create/update a root `docs/evidence/backend-conformance/README.md` that links the
immutable run and explains that any later backend-semantic/corpus change returns
status to `evidence_required` until another separately approved run.

Run the already accepted software-track generator explicitly:

```bash
python3 -B scripts/write-backend-conformance-closure.py \
  --evidence docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid> \
  --out docs/evidence/backend-conformance/hardware-closure.json
```

It reads evidence and writes deterministic closure bytes plus the root
`SHA256SUMS`; no manually entered pass is permitted. After maintained
documentation is final, rerun it once so the path-sorted root checksum manifest
covers the final README/closure/evidence tree, then prove the second run is
byte-idempotent. The closure identifier/owner/projection were registered by the
software track and are read-only here.

### 2. Maintained documentation

Update:

- architecture runtime/fitness notes with the exact qualifying commit, device
  and evidence link;
- the contract-governance software closure handoff with a link to the separate
  hardware closure, without rewriting its frozen pending verdict;
- roadmap/evidence indexes to mark this exact gate answered; and
- `Runtime.canvas` with a compact evidence link/status only if it remains
  non-overlapping and readable.

Do not claim:

- continuous GPU CI;
- portability to another device/driver/commit;
- any speedup or performance qualification; or
- that future semantic changes inherit this evidence.

### 3. Exercise the prepared freshness rule

Run the already accepted backend-conformance checker against the new closure.
It must verify retained checksums and the evidence commit/corpus/binary/device
tuple, then report `qualified_exact_revision` for the exact matching revision.
Synthetic tests from the software track already prove later source/corpus drift
becomes `evidence_required`; this PRD does not edit checker behavior.

### 4. Final checks

Run all evidence, architecture, parameter, corpus, Rust, CUDA-feature compile,
determinism, Lean and full repository checks offline. Confirm no Terraform
plan/state, credentials or paid-resource data is tracked.

## Allowed files

- `docs/evidence/backend-conformance/hardware-closure.json` (new)
- `docs/evidence/backend-conformance/README.md` (new or update)
- `docs/evidence/backend-conformance/SHA256SUMS` (new)
- `docs/evidence/README.md`
- `docs/architecture/runtime.md`
- `docs/architecture/fitness-functions.md`
- `docs/architecture/README.md`
- `docs/architecture/Runtime.canvas`
- `docs/ROADMAP.md`
- `docs/prds-contract-governance/README.md`
- `docs/prds-contract-governance-gpu/README.md`

## Non-goals

- No cloud/network/credential/Terraform operation or second device run.
- No modification of qualifying evidence bytes.
- No production code, contract, corpus, fixture, semantic or threshold change.
- No continuous-GPU-CI, cross-device or performance claim.
- No concealment of future evidence staleness.
- All `scripts/**`, registry and generated-index paths invoked by this PRD are
  read-only accepted software-track tools; this PRD may not edit them.

## Test guidance

Run:

```bash
python3 -B scripts/run-backend-conformance.py --validate \
  docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid>/cuda-report.json
python3 -B scripts/write-backend-conformance-closure.py \
  --evidence docs/evidence/backend-conformance/<repository_commit>/<corpus_sha256>/<device_uuid> \
  --out docs/evidence/backend-conformance/hardware-closure.json
sha256sum -c docs/evidence/backend-conformance/SHA256SUMS
python3 -B scripts/check-backend-conformance.py
python3 -B scripts/check-contract-governance-closure.py
python3 -B scripts/check-artifact-registry.py
python3 -B scripts/check-architecture-canvases.py
bash scripts/check-rust.sh
bash scripts/check-determinism.sh
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

## Acceptance criteria

1. The accepted deterministic closure generator produces the exact strict
   hardware closure and final path-sorted root checksum manifest idempotently
   from immutable evidence and refuses every non-qualifying branch; no manually
   asserted pass or source edit occurs.
2. Maintained docs identify the exact commit/corpus/device scope and make no
   continuous, portable or performance claim.
3. Normal CI verifies evidence integrity and reports exact-revision qualification
   versus evidence-required status truthfully.
4. Future semantic/corpus changes cannot inherit the current qualification
   silently.
5. The qualifying evidence bytes remain unchanged and no cloud/private/plan/
   state material is tracked.
6. All evidence, architecture, Rust, determinism, Lean and full repository checks
   pass offline.
