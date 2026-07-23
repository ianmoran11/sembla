# PRD 0001: Remove generated agent artifacts and the machine-local symlink

max_review_cycles: 3

## Context

Read this folder's `README.md` first. At the audited baseline, Git tracks 138
files under `.pi-subagents/` totaling about 19.6 MiB. They are generated inputs,
outputs, metadata, and full JSONL transcripts; keeping them in source control
adds clone/review noise and can preserve prompts, local paths, or future secrets.
The root also tracks `ltmain.sh` as a symlink to the absolute Homebrew path
`/opt/homebrew/Cellar/libtool/2.5.4/share/libtool/build-aux/ltmain.sh`. No tracked
project file references it, and the target is invalid on Linux and most Macs.

`.piprd/**` is different: it is managed workflow state and must not be cleaned by
this PRD.

## Goal

New clones contain neither generated `.pi-subagents` artifacts nor a
machine-specific `ltmain.sh`, and future agent artifacts stay untracked without
changing history or deleting managed/scientific evidence.

## Requirements

1. Before removal, use `git grep` outside `.pi-subagents/` to verify no tracked
   source or durable documentation references a file inside that directory. If
   a genuinely durable conclusion is referenced, move only that conclusion to
   an appropriately named `docs/` file and update the reference; never preserve
   raw transcripts merely for provenance.
2. Remove all tracked `.pi-subagents/**` files from the repository and add the
   root-anchored ignore rule `/.pi-subagents/`.
3. Remove the tracked `ltmain.sh` symlink. Confirm it has no repository caller.
   Add `/ltmain.sh` to `.gitignore` only if a documented local tool regenerates
   it during ordinary work; otherwise deletion alone is preferred.
4. Do not modify or remove `.piprd/**`, `.piprd-config.json`, fixtures, curated
   calibration/GPU artifacts, or vendored spike sources.
5. Do not rewrite Git history. Record in implementation notes that historical
   blob removal, if ever desired for security, requires a separate explicit
   migration and force-push plan.

## Allowed files

- `.gitignore`
- `ltmain.sh` (deletion only)
- `.pi-subagents/**` (deletion only)
- A narrowly scoped `docs/` preservation file only if requirement 1 proves one
  is necessary
- Managed implementation notes/artifacts

## Non-goals

- Cleaning `.piprd/**`.
- Deleting local build caches.
- `git filter-repo`, BFG, rebasing, or force-pushing.
- Removing curated evidence under `calibration/`, `fixtures/`, or `spikes/`.

## Implementation notes

Treat this as an index-cleanup commit, not a content migration. Use
`git diff --summary` and an outside-directory reference scan to explain the
large deletion set. Record baseline/final tracked counts and bytes.

## Test and check guidance

Run:

```bash
test -z "$(git ls-files .pi-subagents)"
test ! -e ltmain.sh
git check-ignore .pi-subagents/example-transcript.jsonl
./scripts/check.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

Inspect `git diff --stat` deliberately: the large deletion set is expected, but
no path outside the allowed list should appear.

## Acceptance criteria

1. `git ls-files .pi-subagents` is empty and a probe path under
   `.pi-subagents/` is ignored.
2. `ltmain.sh` is absent from both the working tree and Git index; `git grep`
   finds no dangling reference.
3. `.piprd/**` and every protected scientific/evidence path are unchanged.
4. The implementation notes record the pre-PRD HEAD; review observes the same
   HEAD as the diff base, the pending diff is confined to allowed paths, and no
   history-rewrite tooling or instructions were added.
5. Repository, parity, and diff checks pass.
