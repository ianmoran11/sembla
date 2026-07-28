# PRD 0011 Review — Attempt 1

## Assessment

**REVISE**

## Blocking issues

1. The SVG edges terminate at node centers in `frontend/Sembla/WidgetDisplay.lean`, and node circles are rendered after edges. The radius-25 circles therefore cover the small end markers, leaving the visible graph without arrowheads. End edges at circle boundaries or otherwise ensure direction markers remain visible.
2. `frontend/README.md` still states that the package has no external Lake dependencies, contradicting the new pinned ProofWidgets dependency. Update that setup text.

## Acceptance criteria

1. **PASS:** Pinned ProofWidgets build and PRD-0010 parity checks pass.
2. **REVISE:** Exact graph props and JSON tests pass, but visible SVG directionality is obscured.
3. **PASS:** Recover probability, aggregate no-plot behavior, beta density samples, and priorless behavior are tested and pass.
4. **PASS:** Pure prop builders are IO-free; only the thin registration layer uses `CoreM`.
5. **PASS:** Manual instructions provide exact file, lines, cursor tokens, and expected output, subject to the arrow-rendering defect.
