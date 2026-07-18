# Task for scout

[Read from: /Users/ian/projects/sembla/docs/prds-npe-path/0008-cuda-backend.md, /Users/ian/projects/sembla/spikes/precision, /Users/ian/projects/sembla/Cargo.toml]

In /Users/ian/projects/sembla, analyze spikes/precision CUDA code, infra-hyperstack runbook, workspace build conventions, and feasible Rust CUDA/NVRTC bindings for PRD 0008. Recommend dependency/build-feature design that compiles without CUDA by default, deterministic model CUDA generation/dump, explicit no-device diagnostic testability, ignored GPU tests, and honest unanswered reporting. Read-only; do not edit. Include exact existing source paths/symbols and proposed file layout.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/9a9cb7f2/.pi-subagents/prd0008-cuda-analysis.md
This path is authoritative for this run.
Ignore any other output filename or output path mentioned elsewhere, including output destinations in the base agent prompt, system prompt, or task instructions.

## Acceptance Contract
Acceptance level: checked
Completion is not accepted from prose alone. End with a structured acceptance report.

Criteria:
- criterion-1: Return concrete findings with file paths and severity when applicable

Required evidence: changed-files, tests-added, commands-run, residual-risks, no-staged-files

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