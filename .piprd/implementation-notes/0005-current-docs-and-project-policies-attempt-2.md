# PRD 0005 revision — attempt 2

## Review blocker resolution

- Left the accepted PRD implementation in `README.md`, `docs/sembla-assessment.md`, `CONTRIBUTING.md`, and `SECURITY.md` unchanged.
- Preserved the concurrently edited, unrelated local draft at `docs/australian-population-use-case.md` without reading it as implementation input, editing it, moving it, staging it, or deleting it.
- Added the exact path `/docs/australian-population-use-case.md` to the repository-local `.git/info/exclude`. This local, untracked exclusion prevents piprd's repository-wide `git add -A` from absorbing the draft while allowing its owner to continue editing it.
- The exclusion is intentionally temporary and must remain until the entire managed hygiene `/piprd` run completes. Remove the exact entry afterward so the local draft is not forgotten.

## Commit-candidate evidence

- `git check-ignore -v docs/australian-population-use-case.md` resolves to `.git/info/exclude` line 10.
- A temporary-index emulation of piprd's staging (`read-tree HEAD`, `git add -A -- .`, then resetting `.piprd` and `.piprd-config.json`) produced exactly:
  - `CONTRIBUTING.md`
  - `README.md`
  - `SECURITY.md`
  - `docs/sembla-assessment.md`
- The real Git index remains empty.

## Revalidation

- `gh api repos/ianmoran11/sembla/private-vulnerability-reporting --jq .enabled` — `true`.
- Temporary read-only Markdown audit — 154 files, 98 local paths, 5 cross-file anchors; all resolved.
- `./scripts/check.sh` — passed.
- `bash frontend/scripts/check-parity.sh` — passed directly.
- `git diff --check`, `git diff --cached --check`, and `git diff HEAD --check` — passed; the two new policy files have no trailing whitespace.
- Compared 50 protected `Cargo.lock`, NPE artifact, fixture, and example files byte-for-byte with `HEAD`; none changed.
