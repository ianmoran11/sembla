# PRD 0008 implementation notes — attempt 1

## Baseline and scope

- Baseline commit: `857ffa6d2b3b52b1f8577e1f62126256c3ed3579`.
- Read `docs/prds-project-hygiene/README.md` and the active PRD before editing.
- Inspected HEAD, staged/unstaged changes, untracked files, `.piprd/run-state.json`, cache ignore rules, actual cache locations, protected paths, and the repository-local exclusion protecting `docs/australian-population-use-case.md`.
- The real Git index remains empty. Existing `.piprd/**` changes are managed run state and were not used as implementation input.
- The implementation candidate contains exactly four allowed files:
  - `.gitignore`
  - `CONTRIBUTING.md`
  - `scripts/clean-local.sh`
  - `scripts/tests/test_clean_local.py`

## Implementation

- Added an executable strict-mode `scripts/clean-local.sh` with caller-CWD-independent root discovery.
- Default mode is a size-reporting dry run; deletion requires the sole `--apply` option. `--help` is supported and unknown/multiple options fail.
- The closed allowlist contains only root `target/`, `frontend/.lake/`, root `.pytest_cache/`, `calibration/npe/.venv/`, and discovered `__pycache__` directories below `calibration/npe/`.
- Discovery uses `find -P`, prunes the virtual environment and protected source/evidence names, and deletion uses only exact candidate paths. Nested symlinks are not followed.
- Script and repository paths are canonicalized; symlinked script/repository invocation is rejected. Every existing candidate is canonicalized, required to retain its exact allowlisted identity below the root, checked against the protected denylist, and checked with `git ls-files` before any removal. All candidates are preflighted before an apply begins and checked again immediately before removal.
- The denylist covers `.git`, `.piprd`, `.pi-subagents`, fixtures, examples, calibration artifacts, spike artifacts/evidence, generic Terraform names/extensions, and the repository's actual Terraform roots: `spikes/precision/infra/`, `infra-hyperstack/`, and `infra-vultr/`.
- Added temporary synthetic-repository tests for dry-run, apply, idempotence, nested symlink no-follow, symlink escape, protected paths, actual Terraform roots, tracked files, unknown flags, help, unrelated caller CWD, and symlinked repository roots.
- After review reproduced an ambient Git-environment bypass, added fail-closed checks for non-empty `GIT_INDEX_FILE`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, and `GIT_IMPLICIT_WORK_TREE` before repository discovery. `--help` remains available before these guards.
- Extended the synthetic tracked-file coverage to control the default Git environment and prove every guarded override fails without deleting a normally tracked cache file. Both the alternate-index and alternate-`GIT_DIR`/`GIT_WORK_TREE` reproductions now exit 1 and preserve `target/tracked.txt`.
- Added the missing repository-owned `/.pytest_cache/` ignore rule.
- Documented preview-first usage, explicit apply, bounded scope, and Rust/Lean rebuild plus Python reinstall costs in `CONTRIBUTING.md`.

## Validation

No `--apply` invocation was run in the developer's real checkout.

Passed:

```text
bash -n scripts/clean-local.sh
python3 -B -m unittest scripts.tests.test_clean_local
  Ran 9 tests ... OK
bash scripts/clean-local.sh
  reported target 1.7G, frontend/.lake 315M, .pytest_cache 20K,
  calibration/npe/.venv 887M, and two NPE __pycache__ directories;
  ended "Dry run complete; no files were removed."
bash scripts/clean-local.sh --help
./scripts/check.sh
  complete documentation, Rust, Lean proof-hygiene, parity, and lock checks passed
bash frontend/scripts/check-parity.sh
git diff --check
```

Additional evidence:

- An independent adversarial review returned `APPROVED` after confirming the actual Terraform roots are hard-coded and behaviorally tested.
- After the revision, a temporary-index candidate containing the four explicit implementation files passed `git diff --cached --check` and wrote tree `76c8b10d7d86d31d7ee69b99fe90c1a2f4523400` without touching the real index.
- All 110 checked frozen/protected files were byte-identical to `HEAD`.
- `Cargo.lock` remains byte-identical with SHA-256 `410a96ce65551f3f5f6901b3e972575090ab5253a0449643564f6692d40bbd2e`.
- No Python cache artifacts remain under `scripts/`.
- `docs/australian-population-use-case.md` remains present and protected by its single repository-local `.git/info/exclude` rule.
- Eight unrelated untracked paths under `docs/prds-demographic-slots/` appeared after validation. They were not read, modified, staged, ignored, or included in the explicit PRD 0008 candidate.
