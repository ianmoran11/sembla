# PRD 0003 implementation notes — attempt 1

## Baseline and scope

- Pre-PRD HEAD: `32aad5cb21e36bcc3b95fc14193a9393db253f0f`
  (`Implement 0002-reproducible-check-contract`).
- The active `.piprd/**` log, run state, lock, snapshots, prior implementation
  notes, and review artifacts were already modified or untracked managed state;
  implementation did not edit or absorb them.
- No workflow permissions, runner classes, job commands, scientific inputs, or
  evidence files were changed.

## Upstream action resolution

On 2026-07-23, each mutable major tag and its corresponding exact release tag
were resolved directly from the named upstream Git repository with:

```sh
git ls-remote --tags https://github.com/OWNER/REPOSITORY.git \
  'refs/tags/MAJOR' 'refs/tags/MAJOR^{}' \
  'refs/tags/RELEASE' 'refs/tags/RELEASE^{}'
```

The reviewed results were:

| Upstream repository | Major / exact release | Upstream commit pinned |
| --- | --- | --- |
| `actions/checkout` | `v4` / `v4.4.0` | `11d5960a326750d5838078e36cf38b85af677262` |
| `actions/setup-python` | `v5` / `v5.6.0` | `a26af69be951a213d495a4c3e4e4022e16d87065` |
| `dorny/paths-filter` | `v3` / `v3.0.3` | `d1c1ffe0248fe513906c8e24db8ea791d46f8590` |
| `actions-rust-lang/setup-rust-toolchain` | `v1` / `v1.17.0` | `166cdcfd11aee3cb47222f9ddb555ce30ddb9659` |
| `leanprover/lean-action` | `v1` / `v1.5.0` | `38fbc41a8c28c4cbaec22d7f7de508ec2e7c0dd9` |

For `dorny/paths-filter`, `refs/tags/v3` is annotated: `git ls-remote`
returned tag object `6852f92c20ea7fd3b0c25de3b5112db3a98da050` and peeled it to
`d1c1ffe0248fe513906c8e24db8ea791d46f8590`; the exact `v3.0.3` tag resolves
directly to that peeled commit. The other major and exact release refs returned
the pinned commit directly. A second automated pass read every workflow release
comment, queried that exact upstream tag (including `^{}`), and confirmed all
five unique pinned SHAs matched the upstream response.

## Implementation

- Replaced every non-local `uses:` reference in both workflows with the reviewed
  full commit SHA and an adjacent exact-release comment.
- Added a five-minute timeout to the path-detection job and dispatch-only GPU
  runbook job without changing permissions, runners, commands, or triggers.
- Extended the NPE filter with its smoke harness, defining workflow, direct
  requirements input, and planned `requirements-ci.lock` path while preserving
  both existing broad filters.
- Added `.github/dependabot.yml` with one grouped weekly `github-actions`
  update entry and no other dependency ecosystem.
- Extended `scripts/check-workflow-yaml.rb` to parse YAML recursively, validate
  immutable action pins and release comments, permit only documented `./` local
  action paths, assert NPE self-test paths and short timeouts, retain trigger
  checks, protect minimal permissions, and validate the action-only Dependabot
  policy.
- Updated `docs/ci.md` with the immutable-action, self-testing-filter,
  Dependabot, timeout, and local-action policies.

## Verification

- `ruby scripts/check-workflow-yaml.rb`: passed for both workflows and
  `.github/dependabot.yml`.
- Checker mutation tests passed: mutable tag, missing release comment, missing
  NPE harness path, extra Dependabot ecosystem, and GPU `push` trigger were each
  rejected while the unmodified configuration passed.
- `rg -n 'uses:' .github/workflows` plus a parsed audit found 12 references;
  every one has a full 40-character SHA and exact-release comment.
- Exact-tag upstream resolution recheck: all five unique action pins matched
  their named repository and release tag.
- Parsed filter behavior selected the harness, workflow, requirements input,
  future CI lock, calibration code, and historical NPE PRD examples while
  representative unrelated paths remained skipped.
- `actionlint`: **UNANSWERED** because it is not installed locally; no pass is
  claimed.
- `./scripts/check.sh`: passed.
- `bash frontend/scripts/check-parity.sh`: passed directly.
- `Cargo.lock`, protected artifacts, and scientific evidence remain unchanged;
  `git diff --check`, staged/HEAD whitespace checks, Ruby syntax, and no-index
  checks for both new files passed.
