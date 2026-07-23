# PRD 0005: Repair stale status docs and add contribution/security policies

max_review_cycles: 3

## Context

Read this folder's `README.md` first. The root README's workspace list omits
`crates/sembla-cuda`. `docs/sembla-assessment.md` is a dated assessment but is
not prominently marked historical; it now makes false current-sounding claims,
including “no LICENSE”, “no CI”, and “exactly one external dependency”. The
repository has dual licenses, two workflows, and the runtime allowlist permits
`sha2` plus exact-pinned `libm`. A multi-language project with frozen scientific
artifacts also lacks concise contribution and private security-reporting
instructions.

## Goal

Readers can distinguish historical assessment from current project status,
workspace/dependency claims are accurate, and contributors have one focused
source for checks, frozen-artifact discipline, and security reporting.

## Requirements

1. Add a prominent dated historical-snapshot notice near the top of
   `docs/sembla-assessment.md`. Preserve the assessment as historical analysis,
   but add a concise errata/current-status paragraph correcting objectively
   superseded operational claims and linking to `README.md`, `docs/ROADMAP.md`,
   and `docs/ci.md`. Do not rewrite opinions merely because the project evolved.
2. Update the root README workspace layout to include `crates/sembla-cuda` and
   verify all listed crate descriptions against their manifests/current roles.
3. Add `CONTRIBUTING.md` covering pinned Rust/Lean/Python setup, the canonical
   checks established by PRD 0002, determinism/parity checks, frozen artifact
   policy, explicit regeneration authorization, PRD workflow expectations, and
   a prohibition on committing `.pi-subagents` transcripts or manually staging
   active `.piprd` runtime files. Clarify that existing managed `.piprd`
   records are not to be manually rewritten or cleaned during a run.
4. Confirm the folder-level precondition that GitHub Private Vulnerability
   Reporting is enabled, then add `SECURITY.md` naming the repository's private
   advisory route (`https://github.com/ianmoran11/sembla/security/advisories/new`),
   what details to include, supported-version expectations, and explicit
   guidance never to commit cloud/API credentials, Terraform state/plans,
   console passwords, or raw agent transcripts. Do not add an email address or
   fallback public-issue route.
5. Ensure license prose continues to point at both `LICENSE-APACHE` and
   `LICENSE-MIT`; do not add a redundant generic `LICENSE` unless explicitly
   required by tooling.
6. Cross-link contribution/security guidance from the root README without
   turning it into a policy dump.

## Allowed files

- `README.md`
- `docs/sembla-assessment.md`
- `CONTRIBUTING.md` (new)
- `SECURITY.md` (new)
- `docs/ci.md` only for a broken/current cross-reference
- Managed implementation notes/artifacts

## Non-goals

- A code of conduct or changelog without a demonstrated current need.
- Rewriting technical architecture/design documents.
- Changing licenses, dependency policy, code, tests, or workflows.
- Updating volatile line/test counts throughout historical prose.

## Implementation notes

Prefer a historical banner plus a small current-status errata over repeatedly
rewriting a dated assessment. Keep contributor/security documents concise and
link to authoritative technical docs rather than duplicating them.

## Test and check guidance

Mechanically search for the stale current-sounding claims and inspect every new
link. Run the dependency-free Markdown link checker introduced by PRD 0006 if
available; because this PRD precedes it, otherwise use a temporary read-only
local-link audit and record the command. Then run shared repository/parity/diff
checks.

## Acceptance criteria

1. The historical assessment is unmistakably dated/superseded and no longer
   presents “no CI/no license/one dependency” as current truth.
2. The README workspace layout includes all four workspace crates, including
   `sembla-cuda`, and its policy links resolve.
3. `CONTRIBUTING.md` gives reproducible setup/check instructions and precise
   frozen-artifact/managed-run rules.
4. `gh api repos/ianmoran11/sembla/private-vulnerability-reporting --jq
   .enabled` returns `true`, and `SECURITY.md` provides the corresponding private
   advisory path plus repository-specific credential/artifact guidance.
5. All local Markdown links resolve; repository, parity, and diff checks pass.
