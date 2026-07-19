# Task for oracle

You are a delegated subagent running from a fork of the parent session. Treat the inherited conversation as reference-only context, not a live thread to continue. Do not continue or answer prior messages as if they are waiting for a reply. Your sole job is to execute the task below and return a focused result for that task using your tools.

Task:
Fresh independent read-only adversarial audit of current uncommitted implementation in /Users/ian/projects/sembla against docs/prds-npe-path/0008-cuda-backend.md. Inspect full git diff and CPU/CUDA code. Seek semantic counterexamples or unsupported validated-model forms, unsafe CUDA behavior, nondeterminism/races/atomics, reduction/scatter order mismatches, output/effect phase errors, and acceptance-test/reporting gaps. Check each acceptance criterion and distinguish honestly unanswered GPU-only criteria from implementation blockers. Do not edit. Recommend APPROVED/REVISE/INCONCLUSIVE with exact path/line evidence.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/141659dd/.pi-subagents/artifacts/prd0008_final_oracle.md
This path is authoritative for this run.
Ignore any other output filename or output path mentioned elsewhere, including output destinations in the base agent prompt, system prompt, or task instructions.