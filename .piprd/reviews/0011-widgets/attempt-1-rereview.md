# PRD 0011 Re-review — Attempt 1

## Assessment

**REVISE**

## Blocking issue

- Root `README.md` still describes `frontend/` as a “dependency-free Lean DSL,” but the frontend now directly depends on pinned ProofWidgets4. Update this repository-level description to distinguish the no-mathlib/minimal-dependency frontend from its sole direct ProofWidgets dependency.

## Acceptance criteria

1. **PASS:** Pinned ProofWidgets build and PRD-0010 parity checks pass.
2. **PASS:** Exact graph props and JSON assertions pass; directed arrowheads now remain visible outside node circles.
3. **PASS:** Probability, aggregate no-plot, LogNormal density, and priorless tests pass.
4. **PASS:** Pure prop builders remain IO-free and simulation-free; effects are confined to panel registration.
5. **PASS:** Manual instructions are precise and reference current source lines.

The previous SVG directionality blocker and frontend README contradiction are resolved. No Rust changes were introduced.
