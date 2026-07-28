# PRD 0005 implementation — attempt 1

## Baseline and scope

- Baseline: `4673ea5ae55e5f3013c21b007296b455b188704b` (`Implement 0004-python-ci-lock`).
- Active managed run: `2026-07-23T06-27-55-642Z`, PRD index 4.
- Existing `.piprd/**` changes and untracked records were left as managed run state; no files were staged.
- Implementation files: `README.md`, `docs/sembla-assessment.md`, `CONTRIBUTING.md`, and `SECURITY.md`.
- After final validation, an unrelated untracked `docs/australian-population-use-case.md` appeared concurrently (SHA-256 `ee891b3d592bc8c4703e7edd3cbe8b916e9d25d2673258d9a72af48da8b5d4f0`). It was absent from the initial and implementation-scope status audits, is outside this PRD, and was not used as an implementation input, edited, staged, or deleted; exclude it from PRD 0005 review/commit scope.

## Implementation

- Marked the 18 July 2026 assessment as a historical snapshot and added a 23 July 2026 errata correcting the superseded license, CI/workflow, and runtime-dependency claims while preserving the original assessment text.
- Updated the README to list all four Cargo workspace crates with descriptions checked against their current source roles, and added concise contribution/security links.
- Added pinned Rust 1.79.0, Lean 4.13.0, and Linux CPython 3.12.8 setup/check guidance; determinism/parity expectations; frozen-artifact authorization rules; and managed `.piprd`/`.pi-subagents` discipline.
- Confirmed GitHub Private Vulnerability Reporting is enabled and documented only the private advisory route, report contents, current-version expectations, and repository-specific secret/artifact exclusions.
- Preserved links to both `LICENSE-APACHE` and `LICENSE-MIT`; did not add a generic `LICENSE`.

## Validation

- `gh api repos/ianmoran11/sembla/private-vulnerability-reporting --jq .enabled` — `true`.
- Temporary read-only Python Markdown audit (normal local Markdown links across all tracked Markdown plus the two new policy files, with cross-file anchor validation) — 154 files, 98 local paths, 5 cross-file anchors; all resolved. No PRD 0006 repository checker exists yet.
- Mechanical stale-claim search confirmed the old “no LICENSE”, “no CI”, and “exactly one external dependency” text remains only inside the explicitly dated historical assessment and is directly corrected in the top errata.
- `./scripts/check.sh` — passed.
- `bash frontend/scripts/check-parity.sh` — passed directly.
- `git diff --check`, `git diff --cached --check`, and `git diff HEAD --check` — passed; new untracked policy files also have no trailing whitespace.
- Compared 50 protected `Cargo.lock`, NPE artifact, fixture, and example files byte-for-byte with `HEAD`; none changed.
- Scope audit found no non-managed changed path outside the PRD allowlist; staged diff remains empty.
