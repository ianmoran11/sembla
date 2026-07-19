# Task for oracle

[Read from: /Users/ian/projects/sembla/crates/sembla-cuda/src/codegen.rs, /Users/ian/projects/sembla/crates/sembla-cuda/src/backend.rs, /Users/ian/projects/sembla/crates/sembla-cuda/tests/gpu_semantics.rs, /Users/ian/projects/sembla/crates/sembla-runtime/src/eval.rs, /Users/ian/projects/sembla/crates/sembla-runtime/src/executor.rs]

Perform a read-only semantic parity audit of the latest uncommitted CUDA changes in /Users/ian/projects/sembla. Inspect git diff and relevant CPU evaluator/executor code. Try to find counterexamples in aggregate-use classification/activation, phase state selection, error-buffer handling, nested aggregate dependency ordering, shared scheduling/effect/output uses, zero-row cases, and input-table ordered numeric comparison conversion. Do not edit. Return blockers first with exact path/line evidence; explicitly say if no code blocker is found and distinguish unanswered real-GPU execution.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/237337c8/.pi-subagents/artifacts/fresh_aggregate_oracle.md
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