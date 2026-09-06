# Local CUDA-priority validation — 2026-09-06

This is CPU host-loading evidence on an Apple M2 Pro, not GPU evidence.
`loading/summary.json` identifies the exact baseline and measured candidate.
`loading/raw.json` retains every warmup and measured arm. The benchmark uses
1M and 10M synthesized states, one tick, seed 9009 and grouped observations;
three measured pairs alternate order after warming both binaries. All output
trees match byte-for-byte within every pair, including manifests and hashes.
Large generated input states are omitted; their SHA256 values and generated
models are retained. `benchmark-loading.py` contains the executed commands and
uses each child process's own wait4 resource accounting (macOS RSS is bytes).

The required Rust and determinism checks passed in the developer checkout.
The subsequently added snapshot regression and final all-feature clippy/CUDA
library checks passed separately. CUDA host tests: 41 passed, four hardware
tests ignored. No CUDA device was available locally.

The collector smoke test executes the actual new repeat loop with stand-in
binaries. It verifies warmup/pair ordering and rejects a corrupted output,
without invoking a GPU or creating cloud resources. Twelve collector flag
checks also pass. This checks orchestration, not GPU performance.
