# Concurrent CUDA sweep-draw spike evidence

This targeted paid session ran only the direct concurrency spike at repository
commit `d72057f183d94dd311d21c33c90aaae97200cea6`. It did not rerun the unrelated frozen §L4 gate.

Execution mode: supported --draw-workers on free-running non-blocking CUDA streams with bounded capacity preflight.
Noise/repetition protocol: independent noise with three repetitions plus CRN noise with one repetition.

The `sweep-concurrency/` tree contains workers 1/2/4, three independent-noise repetitions and one CRN repetition
at 1M and 10M, complete output-tree hashes and comparisons, resource samples,
and a negative comparator control and a 1M Nsight Systems CUDA trace exported as CSV. Supported mode additionally
runs the CRN matrix and an oversized-request/no-scientific-output capacity
failure arm. Fused mode additionally requires a capacity-four, two-active-slot
compute-sanitizer shakedown before starting the matrix.
