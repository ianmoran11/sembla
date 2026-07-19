# CUDA differential corpus — 2026-07-19

- PRD 0009 implementation commit: **unanswered — changes were uncommitted during
  implementation**. The remote runner records the exact reviewed commit; replace
  this field only from that captured provenance.
- GPU: **unanswered — no CUDA GPU reachable during implementation**
- Driver: **unanswered — no CUDA GPU reachable during implementation**
- Corpus verdict: **unanswered — GPU tests not run**
- Informational throughput: **unanswered — GPU tests not run**
- ADR full-rate reference: **1,380.5 ticks/sec** for the 26M-row workload shape.

No CUDA result was simulated or inferred from CPU execution. To answer these
fields, provision a remote host using
`spikes/precision/infra-hyperstack/README.md`, then run
`crates/sembla-cuda/scripts/run-differential-corpus.sh` from a clean committed
checkout. Record the resulting commit, GPU model, driver, corpus verdict, and
informational throughput here. Correctness may be established on any
CUDA-capable NVIDIA GPU; a performance statement requires independently
verified full-rate FP64 hardware. Destroy the remote resources after evidence
capture.
