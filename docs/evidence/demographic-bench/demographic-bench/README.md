# Concurrent CUDA sweep-draw spike evidence

This targeted paid session ran only the direct concurrency spike at repository
commit `133f87b8dd31bce9254a1be31fe3a3d3bb772443`. It did not rerun the unrelated frozen §L4 gate.

Execution mode: fused grid-y draw slots in one CUDA phase launch.

The `sweep-concurrency/` tree contains workers 1/2/4, three repetitions at 1M
and 10M, complete output-tree hashes and comparisons, resource samples, a
negative comparator control, a schedule control, and a 1M Nsight Systems CUDA
trace exported as CSV. Fused mode additionally requires a capacity-four,
two-active-slot compute-sanitizer shakedown before starting the matrix.
