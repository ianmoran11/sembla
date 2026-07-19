# Task for reviewer

[Read from: /Users/ian/projects/sembla/docs/prds-npe-path/0008-cuda-backend.md, /Users/ian/projects/sembla/crates/sembla-cuda/src/codegen.rs, /Users/ian/projects/sembla/crates/sembla-cuda/src/backend.rs, /Users/ian/projects/sembla/crates/sembla-cuda/tests/gpu_semantics.rs]

Read-only fresh review of the current uncommitted PRD 0008 implementation in /Users/ian/projects/sembla. Review the full git diff and docs/prds-npe-path/0008-cuda-backend.md, with special focus on the latest aggregate phase/liveness changes and Rows::Input ordered Int/Int f64 promotion. Verify: scheduling aggregates use tick-start state; effect-only aggregates run only for winning rules and before effects; wired-output aggregates use prospective next_state; unwired outputs are not evaluated; shared uses rebuild at required phases; no result-bearing atomics; errors/liveness match CPU; tests cover stale output overflow, transition-only post-effect overflow, inactive and active effect aggregate, unwired and shared aggregate, and >2^53 input ordering. Do not modify files. Report only actionable findings with path/line evidence, then residual CUDA-hardware uncertainty.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/237337c8/.pi-subagents/artifacts/fresh_aggregate_review.md
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