# Task for reviewer

Fresh-context, read-only PRD review. Read docs/prds-surface-syntax/README.md and docs/prds-surface-syntax/0001-kernel-and-contract.md. Inspect current git status/diff and these implementation files: frontend/Sembla/DSL.lean, frontend/Sembla/SurfaceKernelTests.lean, frontend/Sembla.lean, plus .piprd/reviews/0001-kernel-and-contract/attempt-1/implementation-notes.md. Check all six acceptance criteria and report only concrete blockers, or APPROVED with concise evidence. Do not run the long full suite and do not modify files. Independent checks already returned 0 for lake build, negative suite, parity, ./scripts/check.sh, git diff --check, and frozen-file diff. The exact JSON guard is one long line; inspect its presence and compiled status rather than reproducing it.

## Acceptance Contract
Acceptance level: attested
Completion is not accepted from prose alone. End with a structured acceptance report.

Criteria:
- criterion-1: Return concrete findings with file paths and severity when applicable

Required evidence: review-findings, residual-risks

Finish with a fenced JSON block tagged `acceptance-report` in this shape:
Use empty arrays when no items apply; array fields contain strings unless object entries are shown.
```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "specific proof"
    }
  ],
  "changedFiles": [
    "src/file.ts"
  ],
  "testsAddedOrUpdated": [
    "test/file.test.ts"
  ],
  "commandsRun": [
    {
      "command": "command",
      "result": "passed",
      "summary": "short result"
    }
  ],
  "validationOutput": [
    "validation output or concise summary"
  ],
  "residualRisks": [
    "none"
  ],
  "noStagedFiles": true,
  "diffSummary": "short description of the diff",
  "reviewFindings": [
    "blocker: file.ts:12 - issue found, or no blockers"
  ],
  "manualNotes": "anything else the parent should know"
}
```