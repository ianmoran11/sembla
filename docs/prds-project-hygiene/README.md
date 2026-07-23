# Project hygiene PRDs

Ordered maintenance track for the repository-hygiene findings recorded after the
composition-integration run. Start from the first numbered file; `/piprd run`
continues through the remaining Markdown files in natural order:

```text
/piprd run docs/prds-project-hygiene/0001-generated-artifacts-and-portability.md
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; these constraints bind when a numbered PRD is less specific.

## Preconditions

- Commit this PRD folder before starting. `/piprd run` requires a clean relevant
  working tree.
- Do not absorb active `.piprd/**` run state, logs, reviews, advice, locks, or
  snapshots into an implementation commit. They are managed workflow state.
- Start from commit `d52c892` or a descendant containing the approved
  composition-integration track. If the baseline has moved, re-check every
  numerical/path count in the PRD before treating it as an assertion.
- Before starting the run, the repository owner must enable GitHub Private
  Vulnerability Reporting for `ianmoran11/sembla`. Verify the external setting:

  ```bash
  test "$(gh api repos/ianmoran11/sembla/private-vulnerability-reporting --jq .enabled)" = true
  ```

  PRD 0005 documents that real private route; it must not publish a disabled
  advisory URL or invent another contact.

## Shared constraints

- This track changes repository maintenance surfaces only. It must not change
  simulation, linker, identity, hashing, serialization, RNG, CUDA, Lean DSL, or
  scientific semantics.
- Existing examples, fixtures, CSV/hash goldens, composition bundles, NPE
  evidence, GPU evidence, and generated scientific reports remain byte-frozen.
- No history rewrite is part of this managed run. In particular, removing
  generated files from `HEAD` does not authorize `git filter-repo`, force-pushes,
  or deletion of remote history.
- `.piprd/**` is intentionally managed and is not the target of the
  `.pi-subagents/**` cleanup in PRD 0001.
- Do not add runtime Rust/Lean dependencies. Tooling additions must be narrowly
  scoped, pinned, and justified by the PRD that introduces them.
- Preserve the dual `LICENSE-APACHE` / `LICENSE-MIT` licensing choice.
- Do not regenerate locks or evidence as a side effect. Lock changes must be the
  explicit output of the lock-focused PRD and must be reproducible.

## Required checks

Unless a numbered PRD adds stricter checks, implementation and review run from
the repository root:

```bash
./scripts/check.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

When a PRD intentionally changes the check entry points, run both the new
entry points and the previous equivalent commands during that PRD so coverage
cannot silently shrink.

## Run order

1. `0001-generated-artifacts-and-portability.md` — untrack generated subagent
   transcripts and remove the machine-specific root symlink.
2. `0002-reproducible-check-contract.md` — make Cargo checks lock-strict and
   separate Rust-only from complete repository validation.
3. `0003-ci-supply-chain-and-filters.md` — pin Actions by SHA, automate their
   maintenance, and make path-filtered smoke tests self-testing.
4. `0004-python-ci-lock.md` — add a hashed, transitive Linux/Python CI lock for
   the NPE smoke environment without changing scientific versions or evidence.
5. `0005-current-docs-and-project-policies.md` — repair stale status claims and
   add focused contribution/security guidance.
6. `0006-text-policy-and-doc-link-check.md` — establish cross-platform text
   policy and a dependency-free local Markdown-link check.
7. `0007-cargo-package-policy.md` — prevent accidental publication and complete
   workspace package metadata.
8. `0008-safe-local-cleanup.md` — document and provide an opt-in, bounded local
   cache cleanup command.

Run in order. Later PRDs may build on scripts or documentation introduced by
earlier ones; do not implement later-PRD work early.

## Global non-goals

- Reformatting the repository or normalizing every existing file.
- Rewriting Git history to remove old generated blobs.
- Deleting curated scientific evidence or vendored precision-spike sources.
- Adding release automation, publishing crates, or changing versions.
- Updating numerical dependencies merely because newer versions exist.
- Turning local cleanup into an implicit or destructive `git clean` operation.
