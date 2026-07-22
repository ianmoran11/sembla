# Task for reviewer

[Read from: /Users/ian/projects/sembla/plan.md, /Users/ian/projects/sembla/progress.md]

Read-only formal review of current uncommitted implementation against docs/prds-composition/0009-linker-nesting.md. First read docs/prds-composition/README.md and the PRD completely. Inspect git status/diff and relevant full files. Explicitly assess acceptance criteria 1-6 with concrete file/line evidence, including linker boundary/hiding/transitive semantics, source-map shape/order, laws/negative guards, fixtures/goldens, CLI divergence. Identify blockers only; do not modify files or commit. Note that current workspace includes the narrow CLI initializer change needed to make repeated two_regions population-file runs active, even though main.rs is outside the original allowed list; judge both literal scope and functional necessity.

---
**Output:**
Write your findings to exactly this path: /tmp/prd0009-code-review.md
This path is authoritative for this run.
Ignore any other output filename or output path mentioned elsewhere, including output destinations in the base agent prompt, system prompt, or task instructions.

## Acceptance Contract
Acceptance level: reviewed
Completion is not accepted from prose alone. End with a structured acceptance report.

Criteria:
- criterion-1: Return concrete findings with file paths and severity when applicable

Required evidence: changed-files, tests-added, commands-run, validation-output, residual-risks, no-staged-files

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