# Task for context-builder

Design a documentation revision plan for /Users/ian/projects/sembla/docs/design/composition-options.md that adds syntax and concrete composed-system examples throughout—not just in one examples section. Review the full document and current repository syntax. Propose a running example (or small consistent family), exact insertion points by line/heading, what each snippet teaches, and how to label current vs proposed vs conceptual ACSet/Catlab notation. Ensure examples cover product, instances, wiring/delay, feedback, nesting/expose/hide, constrained product, synchronized families, heterogeneous schedulers, Options A-D lowering, source graph, laws, tests, and phases. Do not modify project/source files.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/7a0a8bcf-aa1a-48f1-bb04-f91041cedc43/composition-examples/revision-plan.md
This path is authoritative for this run.
Ignore any other output filename or output path mentioned elsewhere, including output destinations in the base agent prompt, system prompt, or task instructions.

## Acceptance Contract
Acceptance level: reviewed
Completion is not accepted from prose alone. End with a structured acceptance report.

Criteria:
- criterion-1: Implement the requested change without widening scope
- criterion-2: Return evidence sufficient for an independent acceptance review

Required evidence: changed-files, tests-added, commands-run, validation-output, residual-risks, no-staged-files

Review gate: required by reviewer.

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