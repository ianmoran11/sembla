# Concurrent CUDA sweep-draw spike evidence

This targeted paid session ran only the direct concurrency spike at repository
commit `8ace75c83755b7bc767ef07c6e7eef05d3c2036b`. It did not rerun the unrelated frozen §L4 gate.

Execution mode: free-running non-blocking CUDA streams without tick barriers.
Noise/repetition protocol: CRN noise with one repetition (correctness arm; timing claims remain with the independent-noise arm).

The `sweep-concurrency/` tree contains workers 1/2/4, one repetition
at 1M and 10M, complete output-tree hashes and comparisons, resource samples,
and a negative comparator control. Fused mode additionally
requires a capacity-four, two-active-slot compute-sanitizer shakedown before
starting the matrix.
