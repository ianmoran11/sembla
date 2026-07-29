# Concurrent CUDA sweep-draw spike evidence

This targeted paid session ran only the direct concurrency spike at repository
commit `b6e42cef6e1e00c740d538ae0163f8d50922b0ca`. It did not rerun the unrelated frozen §L4 gate.

Execution mode: free-running non-blocking CUDA streams without tick barriers.

The `sweep-concurrency/` tree contains workers 1/2/4, three repetitions at 1M
and 10M, complete output-tree hashes and comparisons, resource samples, a
negative comparator control, a schedule control, and a 1M Nsight Systems CUDA
trace exported as CSV.

See [`ANALYSIS.md`](ANALYSIS.md) for the positive verdict: every output tree
matched, real two-stream kernel overlap was confirmed, and free streams posted
the best measured CUDA times — 1.598x/1.745x at 10M two/four workers, beating
both isolated default-stream backends (1.487x/1.658x) and lockstep streams
(1.296x/1.405x). CRN hardware correctness coverage remains required before a
numbered concurrency PRD.
