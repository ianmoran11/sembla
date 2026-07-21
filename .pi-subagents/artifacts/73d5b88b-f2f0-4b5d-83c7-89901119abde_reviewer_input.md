# Task for reviewer

Review the edited /Users/ian/projects/sembla/docs/design/composition-options.md against the user's request: syntax examples and concrete composed-system examples should appear throughout the design document, while current versus proposed versus conceptual syntax must be unmistakable. Check semantic consistency of the running EpidemicPolicy/CareNetwork/CapacitySafeCare examples; one-tick and two-tick feedback timing; ACSet/Topos claims and links; product-versus-ACSet-product distinctions; structured cospan/variable-sharing caveats; option and phase examples; markdown fences/labels; and any internal naming/path contradictions. Compare claimed current syntax with frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean and examples/sir_policy.json. Return only evidence-backed issues with severity and exact edited-file line ranges; do not modify project/source files.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/73d5b88b-f2f0-4b5d-83c7-89901119abde/composition-examples/final-review.md
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