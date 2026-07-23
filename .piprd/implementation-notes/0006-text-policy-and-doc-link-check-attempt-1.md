# PRD 0006 implementation — attempt 1

## Baseline and scope

- Started from `dc1cb4ed14810266ec8ed84b6fa5ce66ff14db48` (`Implement 0005-current-docs-and-project-policies`) with an empty real index.
- Preserved all active `.piprd/**` run state and the owner-authorized local exclusion for the concurrent `docs/australian-population-use-case.md` draft.
- A temporary-index emulation of piprd staging contains exactly the eight allowed implementation files: `.editorconfig`, `.gitattributes`, `.github/workflows/ci.yml`, `CONTRIBUTING.md`, `docs/ci.md`, `scripts/check-markdown-links.py`, `scripts/check.sh`, and `scripts/tests/test_check_markdown_links.py`.

## Implementation

- Added conservative `text=auto eol=lf` Git policy, explicit `-text` protection for scientific/model binaries, images, archives, compiled fixtures, and the tracked mixed-byte bootstrap evidence log. JSON and CSV remain text.
- Added matching UTF-8/LF/final-newline editor defaults, four-space Rust indentation, two-space Lean indentation, and a Markdown hard-break override.
- Added a directly runnable Python-standard-library checker that enumerates tracked Markdown through Git, ignores `.piprd/**`, remote/mailto links, pure anchors, images, inline code, and fenced examples, URL-decodes relative paths, and emits sorted `file:line -> target` failures. It checks the file component of fragment links but intentionally does not validate anchors.
- Added temporary-Git-fixture tests covering literal spaces, URL encoding, fragments, remote/mailto and pure-anchor exclusions, inline code, ordinary fences, fence-like non-closing lines, images, missing targets, deterministic diagnostics, and ignored `.piprd` records.
- Wired the unit tests and repository link scan into strict `scripts/check.sh` and the CI Rust/documentation-hygiene job without a package installation or third-party parser. Documented the direct and canonical paths in `CONTRIBUTING.md` and `docs/ci.md`.

## Line-ending and integrity evidence

- Before policy files, `git ls-files --eol` reported 813 LF entries, 63 entries without line endings, and 3 content-detected binary entries.
- After temporary-index staging, it reported 817 LF entries, 63 entries without line endings, and the same 3 explicit `-text` entries: two `posterior.pt` files and `spikes/precision/evidence/hyperstack-h100-20260718/bootstrap.log`.
- Temporary-index `git diff --cached --numstat` contains only the eight explicit implementation files; no mass renormalization occurred.
- All 50 protected lock, NPE artifact, fixture, and example files remain byte-identical to `HEAD`.

## Validation

- `python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v` — 2 focused tests passed.
- `python3 -B scripts/check-markdown-links.py` — checked 98 local links in 114 tracked non-managed Markdown files.
- `ruby scripts/check-workflow-yaml.rb` — both workflows and Dependabot configuration parsed and policy checks passed.
- `./scripts/check.sh` — complete documentation, Rust, Lean proof-hygiene, parity, and lock checks passed.
- `bash frontend/scripts/check-parity.sh` — passed directly.
- `git diff --check` and temporary-index `git diff --cached --check` — passed.
- `actionlint` — **UNANSWERED** because it is not installed.
- No Python bytecode/cache artifacts remain, and the real index is empty.
