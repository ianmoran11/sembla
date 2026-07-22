# Task for reviewer

[Read from: /Users/ian/projects/sembla/plan.md, /Users/ian/projects/sembla/progress.md]

Independent read-only verification of current repository against PRD 0009. Read binding README + PRD. Focus on artifacts/tests/checks/scope/frozen compatibility: canonical bytes, mailbox/source-map counts/content, parity/walking coverage, no-delay behavior, display rename/permutation/stable-ID laws, every requested visibility negative, actual two_regions CSV north/south divergence and hashes, legacy artifact preservation, and current check commands if practical. Inspect git diff; do not edit or commit. Return criterion-by-criterion PASS/FAIL and exact blockers.

---
**Output:**
Write your findings to exactly this path: /tmp/prd0009-artifact-review.md
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