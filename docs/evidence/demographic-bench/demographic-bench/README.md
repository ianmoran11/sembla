# Concurrent CUDA sweep-draw spike evidence

This targeted paid session ran only the direct concurrency spike at repository
commit `645bb7c2b8da9be063ee48470a440dee29964408`. It did not rerun the unrelated frozen §L4 gate.

Execution mode: synchronized tick boundaries on explicitly non-blocking CUDA streams.

The `sweep-concurrency/` tree contains workers 1/2/4, three repetitions at 1M
and 10M, complete output-tree hashes and comparisons, resource samples, a
negative comparator control, a schedule control, and a 1M Nsight Systems CUDA
trace exported as CSV.
