# PRD 0008: Add an opt-in, bounded local cache cleanup command

max_review_cycles: 3

## Context

Read this folder's `README.md` first. At audit time, ordinary ignored local
state occupied roughly 1.7 GiB under `target/` and 316 MiB under
`frontend/.lake/`, with additional Python virtualenv/cache directories. Cleanup
is currently ad hoc. This repository also contains protected managed state,
curated calibration/GPU evidence, Terraform material, fixtures, and bundles, so
`git clean -xfd` or an overbroad recursive remover is unacceptable.

## Goal

Contributors can preview and explicitly remove only known rebuildable local
caches using a repository-owned command that cannot reach managed state,
scientific evidence, credentials, or fixtures.

## Requirements

1. Add `scripts/clean-local.sh` with strict shell mode and repository-root
   discovery independent of caller cwd.
2. Default behavior is dry-run: print each allowlisted path, whether it exists,
   and its approximate size. Deletion requires an explicit `--apply` flag.
3. Use a closed allowlist of rebuildable paths, initially limited to root
   `target/`, `frontend/.lake/`, root `.pytest_cache/`,
   `calibration/npe/.venv/`, and Python `__pycache__` directories under
   `calibration/npe/`. Do not use `git clean`, broad globs, or follow symlinks.
4. Before deletion, canonicalize each candidate and prove it is under the
   repository root and exactly matches an allowlisted path/category. Refuse when
   the script itself is reached through an unexpected/symlinked repository root.
5. Hard-code and test a protected denylist including `.git`, `.piprd`,
   `.pi-subagents`, `fixtures`, `examples`, `calibration/npe/artifacts`, all
   `spikes/**/artifacts`/evidence, and Terraform state/plan/config paths. The
   script must never delete tracked files; check with `git ls-files` before
   applying.
6. Support `--help`; unknown flags fail. Output must clearly say that Rust/Lean
   and Python dependencies will need rebuilding.
7. Document dry-run and apply usage in `CONTRIBUTING.md`. Do not run `--apply`
   against the developer's real checkout during implementation/review.
8. Add tests using a temporary synthetic repository tree, including symlink
   escape, protected path, tracked-file, unknown-flag, dry-run, and apply cases.

## Allowed files

- `scripts/clean-local.sh` (new)
- Focused tests under `scripts/tests/**` (new)
- `CONTRIBUTING.md`
- `.gitignore` only if a newly tested rebuildable cache lacks an existing rule
- Managed implementation notes/artifacts

## Non-goals

- Cleaning `.piprd/**` or `.pi-subagents/**`.
- Deleting scientific artifacts, fixtures, examples, vendored sources,
  Terraform files, credentials, plans, state, or evidence.
- Running a real cleanup as acceptance evidence.
- Replacing language-native cleanup commands or adding a task-runner dependency.

## Implementation notes

Test by copying the script into a synthetic temporary repository layout so the
production CLI needs no unsafe root-override flag. Prefer repeated explicit
paths over clever recursive discovery, and make repeated cleanup idempotent.

## Test and check guidance

Run the cleanup test suite entirely against temporary directories. In the real
checkout run only:

```bash
bash scripts/clean-local.sh
bash scripts/clean-local.sh --help
./scripts/check.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

Verify dry-run output contains no protected or tracked candidate.

## Acceptance criteria

1. Default invocation performs no deletion; `--apply` is mandatory and confined
   to the closed allowlist.
2. Tests prove refusal for symlink escape, protected paths, tracked files, and
   unknown flags, and prove deletion only in a temporary fixture tree.
3. `.piprd`, fixtures, examples, calibration artifacts, spike evidence,
   Terraform material, and every tracked file are unreachable by the remover.
4. Contribution docs explain cost/rebuild consequences and show dry-run first.
5. Repository, parity, focused cleanup tests, and diff checks pass.
