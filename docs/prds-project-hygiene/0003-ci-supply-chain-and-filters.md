# PRD 0003: Pin CI actions and make filtered jobs self-testing

max_review_cycles: 4

## Context

Read this folder's `README.md` first. Both workflows execute actions through
mutable major tags (`actions/checkout@v4`, `actions/setup-python@v5`,
`dorny/paths-filter@v3`, `actions-rust-lang/setup-rust-toolchain@v1`, and
`leanprover/lean-action@v1`). The workflows use sensible least-privilege token
permissions, but mutable tags remain a supply-chain gap. The NPE job's filter
covers `calibration/**` and historical NPE PRDs, but not its own harness
`scripts/check-npe-smoke.sh` or the workflow that defines it.

## Goal

CI executes reviewed immutable action revisions, receives automated action-only
update proposals, and runs path-filtered checks whenever their implementation
or workflow changes.

## Requirements

1. Resolve each currently used action release tag to its reviewed upstream
   40-character commit SHA. Pin every `uses:` entry in
   `.github/workflows/*.yml` to the full SHA and retain the human release tag in
   an adjacent comment, e.g. `@<sha> # v4.x.y`. Do not invent SHAs; record the
   resolution commands and upstream repository in implementation notes.
2. Add `.github/dependabot.yml` configured only for the `github-actions`
   ecosystem, with a modest weekly cadence and grouped updates where supported.
   Do not enable Cargo, pip, Terraform, or Lean dependency updates in this PRD.
3. Extend the NPE path filter to include `scripts/check-npe-smoke.sh`,
   `.github/workflows/ci.yml`, and the Python lock/input files used by the job.
   Preserve the existing `calibration/**` coverage.
4. Add short `timeout-minutes` values to the path-detection and manual GPU stub
   jobs if absent. Preserve least-privilege permissions and the dispatch-only
   GPU contract.
5. Extend `scripts/check-workflow-yaml.rb` to reject mutable/non-SHA `uses:`
   references, assert the NPE self-test paths, and retain its existing trigger
   checks. Local actions, if introduced later, may use a documented relative
   path exception.
6. Run `actionlint` when available. If it is unavailable locally, report that
   result as unanswered rather than a pass; the repository's own Ruby checker
   must still pass.

## Allowed files

- `.github/workflows/ci.yml`
- `.github/workflows/gpu-differential.yml`
- `.github/dependabot.yml` (new)
- `scripts/check-workflow-yaml.rb`
- `docs/contributing/ci.md`
- Managed implementation notes/artifacts

## Non-goals

- Changing workflow job semantics, runner classes, or GPU evidence claims.
- Dependency upgrades outside GitHub Actions.
- Python transitive/hash locking (PRD 0004).
- New secrets, write permissions, releases, or deployment jobs.

## Implementation notes

Keep release comments beside immutable SHAs so reviews remain readable. The
workflow checker should parse YAML and inspect `uses:` values rather than rely
only on a grep that can miss quoting or nested steps.

## Test and check guidance

Run:

```bash
ruby scripts/check-workflow-yaml.rb
rg -n 'uses:' .github/workflows
if command -v actionlint >/dev/null; then
  actionlint .github/workflows/*.yml
else
  echo 'UNANSWERED: actionlint unavailable'
fi
./scripts/check.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

Review each SHA against the intended action repository/tag. A syntactically
valid 40-character value is not sufficient evidence by itself.

## Acceptance criteria

1. Every non-local `uses:` reference in both workflows is a full 40-character
   commit SHA with a readable release comment, and the checker enforces it.
2. Dependabot is limited to GitHub Actions and has valid configuration.
3. Changing the NPE smoke harness, workflow, requirements input, or CI lock
   selects the NPE job; unrelated paths retain the existing skip behavior.
4. Workflow permissions remain read-only/minimal and the GPU workflow remains
   `workflow_dispatch` only with no claim of GPU execution.
5. Workflow parsing, repository checks, parity, and diff checks pass.
