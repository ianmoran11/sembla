# PRD 0006: Add cross-platform text policy and local documentation-link checks

max_review_cycles: 3

## Context

Read this folder's `README.md` first. The audited checkout currently has no
CRLF/mixed tracked files and a read-only scan found zero broken targets among 88
relative Markdown links. Those are good current states, but the repository has
no `.gitattributes` or `.editorconfig` to preserve line-ending/editor behavior,
and link validity is not checked automatically. The project spans Rust, Lean,
Shell, Ruby, Python, JSON, CSV, Markdown, binary populations, Torch artifacts,
and curated fixture bundles, so an indiscriminate normalization commit would be
unsafe.

## Goal

Future text edits are stable across macOS/Linux editors, binary artifacts are
never normalized, and broken relative Markdown file links fail a small
no-dependency check before merge.

## Requirements

1. Add `.gitattributes` with conservative root policy: text files use LF;
   known binary types (`*.bin`, `*.pt`, images, archives, and other actually
   tracked binary formats) are `-text`. Do not mark JSON/CSV fixtures binary.
2. Add a root `.editorconfig` specifying UTF-8, LF, final newline, and trailing
   whitespace policy. Preserve Markdown hard-break compatibility with a scoped
   override if necessary. Match existing Rust/Lean indentation rather than
   reformatting files.
3. Do not mass-renormalize the tree. Use `git ls-files --eol` before and after;
   any content diff outside the new/explicitly edited files is a blocker.
4. Add a dependency-free script, preferably
   `scripts/check-markdown-links.py`, that enumerates tracked Markdown files,
   ignores `.piprd/**` managed artifacts, skips remote/mailto links and pure
   anchors, decodes relative paths, and fails with `file:line -> target` for
   missing local targets. Avoid false matches inside fenced code and images.
   Anchor-fragment validation is optional and must not be claimed if absent.
5. Wire the link check into the canonical local/CI hygiene path established by
   PRDs 0002–0003. Keep the script directly runnable and document it in
   `CONTRIBUTING.md` and `docs/contributing/ci.md`.
6. Add focused tests for spaces/URL encoding, fragments, fenced examples,
   images, missing targets, and ignored managed paths. Tests must use a temporary
   fixture tree rather than modifying documentation.

## Allowed files

- `.gitattributes` (new)
- `.editorconfig` (new)
- `scripts/check-markdown-links.py` (new)
- Focused script tests under `scripts/tests/**` (new) or an equivalent existing
  test location
- Check entry point/workflow files introduced by PRDs 0002–0003
- `CONTRIBUTING.md`, `docs/contributing/ci.md`
- Managed implementation notes/artifacts

## Non-goals

- Reformatting, line-ending normalization, or whitespace cleanup across existing
  source/fixture files.
- Checking external HTTP availability.
- Requiring a third-party Markdown parser or Node package.
- Editing links that already resolve merely for style.

## Implementation notes

Add policy files first, then inspect—not automatically apply—renormalization.
The link checker should emit deterministic sorted diagnostics and use only the
Python standard library so contributors and CI share the same behavior.

## Test and check guidance

Run focused script tests, then:

```bash
python3 scripts/check-markdown-links.py
git ls-files --eol
./scripts/check.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

Review `git diff --numstat` for any accidental full-file normalization.

## Acceptance criteria

1. `.gitattributes` enforces LF for text and protects every tracked binary
   format; `.editorconfig` expresses matching editor defaults.
2. Existing tracked files are not mass-renormalized and frozen artifacts are
   byte-identical.
3. The link checker passes on the repository, fails on a missing relative
   target, reports useful location evidence, and has focused automated tests.
4. The canonical CI/local path executes the link check without a new dependency.
5. Documentation, repository, parity, and diff checks pass.
