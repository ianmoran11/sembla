# Task for reviewer

[Read from: /Users/ian/projects/sembla/plan.md, /Users/ian/projects/sembla/progress.md]

Fresh read-only PRD review in /Users/ian/projects/sembla. Compare the complete current workspace and git diff against docs/prds-npe-path/0008-cuda-backend.md. Check every acceptance criterion explicitly, including honest unanswered GPU criteria. Inspect implementation and tests, run targeted read-only checks if useful. Do not modify implementation files. Report only evidence-backed blocking issues with exact file/line paths; then give criterion-by-criterion status and recommend APPROVED/REVISE/INCONCLUSIVE. Pay particular attention to arbitrary validated v0.1 models, exact CPU semantics, deterministic reductions/scatters, no result-bearing atomics, NVRTC ABI, state/device residency, hash modes, no fallback, and ignored GPU harness.

---
**Output:**
Write your findings to exactly this path: /Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/141659dd/.pi-subagents/artifacts/prd0008_final_reviewer.md
This path is authoritative for this run.
Ignore any other output filename or output path mentioned elsewhere, including output destinations in the base agent prompt, system prompt, or task instructions.