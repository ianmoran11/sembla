# Concurrent CUDA sweep-draw spike evidence

This targeted paid session ran only the direct concurrency spike at repository
commit `7eb21efd029b09303e7b7d1aede4ccc9c3b30cb5`. It did not rerun the unrelated frozen §L4 gate.

The `sweep-concurrency/` tree contains workers 1/2/4, three repetitions at 1M
and 10M, complete output-tree hashes and comparisons, resource samples, a
negative comparator control, a forced completion inversion, and a 1M Nsight
Systems CUDA trace exported as CSV.
