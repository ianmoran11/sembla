# PRD 0001 implementation notes — attempt 1

## Baseline

- Pre-PRD review HEAD: `476a04962b63051e97b083ca4d1523d7f822812d`.
- The current HEAD tracked 142 files under `.pi-subagents/` with a combined Git
  blob size of 20,667,657 bytes (19.71 MiB): 81 Markdown, 31 JSON, and 30 JSONL
  files. This is four files above the earlier 138-file audit because the PRD
  planning review artifacts were included in the immediately preceding commit.
- The root `ltmain.sh` was a mode-`120000` symlink to
  `/opt/homebrew/Cellar/libtool/2.5.4/share/libtool/build-aux/ltmain.sh`.

## Reference scan

Before removal, `git grep` outside `.pi-subagents/` found nine literal
`.pi-subagents/` mentions in three files. Every hit was a directory-level policy
or cleanup statement in this hygiene track's README, PRD 0001, or PRD 0008; no
tracked source or durable documentation referenced a generated artifact file.
No preservation document was necessary.

The `ltmain.sh` scan found only generated transcript copies and PRD 0001's
intentional description/checks. No source file, build script, workflow, or
durable operational document calls or regenerates it, so no `/ltmain.sh` ignore
rule was added.

## Implementation

- Added the root-anchored `/.pi-subagents/` rule to `.gitignore`.
- Removed all 142 tracked `.pi-subagents/**` files from the current index and
  working tree.
- Removed the machine-local `ltmain.sh` symlink.
- Did not migrate transcript content because the pre-removal scan found no
  durable conclusion referenced from outside the generated directory.

This is current-tree cleanup only. No history rewrite was performed or added.
Historical blob removal, if a future security incident requires it, needs a
separate explicit migration plan covering coordination, backup, rewritten
references, and the required force-push; it is not authorized by this PRD.

## Verification

- Acceptance smoke passed: the current index contains zero `.pi-subagents`
  paths and zero `ltmain.sh` paths; `ltmain.sh` is absent; the ignored probe
  resolves specifically to `.gitignore`'s `/.pi-subagents/` rule; and the
  post-removal caller scan has no hit outside PRD 0001 and this note's
  intentional audit text.
- `./scripts/check.sh` passed.
- `bash frontend/scripts/check-parity.sh` passed, including fixture/bundle
  byte-identity and reproducibility checks.
- `git diff --check`, `git diff --cached --check`, and the combined
  `git diff HEAD --check` passed. The untracked implementation note also passed
  a no-index whitespace check.
- The final index-cleanup summary is 143 deletions: 142 generated files plus
  `ltmain.sh`. The pending tracked diff touches only those paths, `.gitignore`,
  this implementation note, and the two pre-existing active `.piprd`
  modifications.
- The HEAD remains the pre-PRD review HEAD. A before/after SHA-256 comparison
  found zero changes across 304 protected tracked evidence/fixture/example,
  spike, lock, and license files, and zero changes across the four pre-existing
  active `.piprd` files. This note is the only new managed implementation
  artifact.
