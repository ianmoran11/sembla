# PRD 0008 GPU acceptance status

Implementation-time status: **no CUDA GPU was reachable from the development
machine**. No device result is simulated or inferred from CPU execution.

| Criterion | Status | How to answer |
|---|---|---|
| 2. Device Philox equals CPU vectors | **Unanswered — GPU not run** | `crates/sembla-cuda/scripts/run-gpu-tests.sh` runs `device_philox_is_bit_identical_to_checked_cpu_vectors`. |
| 3. SIR, `sir_policy`, and canonical per-tick oracle equality | **Unanswered — GPU not run** | The same script runs the three ignored 200-tick differential tests, including SIR at 100,000 persons. |
| 4. Level A repeatability | **Unanswered — GPU not run** | The same script runs `level_a_same_gpu_run_twice_has_byte_identical_hashes`. |

Provision and validate the remote host with
[`spikes/precision/infra-hyperstack/README.md`](../../spikes/precision/infra-hyperstack/README.md),
run the script from the repository root, preserve the generated provenance,
test log, and `SHA256SUMS`, then follow the runbook's destruction steps.
Correctness may be established on any compatible CUDA GPU. Do not attach a
performance claim unless the host is independently verified as full-rate.
