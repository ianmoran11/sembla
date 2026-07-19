# Task for planner

[Read from: /Users/ian/projects/sembla/context.md]

You are a delegated subagent running from a fork of the parent session. Treat the inherited conversation as reference-only context, not a live thread to continue. Do not continue or answer prior messages as if they are waiting for a reply. Your sole job is to execute the task below and return a focused result for that task using your tools.

Task:
Read-only planning task in /Users/ian/projects/sembla. Analyze docs/prds-npe-path/0008-cuda-backend.md and the three review blockers: model-name CUDA comment injection, CPU sequential reduction semantics changed to two halves, and CPU-vs-CUDA first-error ordering across transitions/claims. Inspect current code/diff and propose the smallest safe next revision that satisfies the existing PRD without changing CPU/IR semantics. Be concrete about CUDA reduction design that exactly preserves sequential f64/int order while remaining deterministic/atomic-free, and about a device-resident transition/claim validation sequence that fixes error precedence. List files, tests, risks, and checks. Do not edit.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/05ed1b28/.pi-subagents/artifacts/prd0008_next_revision_planner.md
This path is authoritative for this run.
Ignore any other output filename or output path mentioned elsewhere, including output destinations in the base agent prompt, system prompt, or task instructions.