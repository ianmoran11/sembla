# Focused CUDA readback/contended-kernel diagnostic evidence

This targeted paid session ran only the fixed 10M grouped CUDA diagnostic at
repository commit `e9600f31251c495268337ed6c0eb16d7fb2b838c`. It did not rerun the unrelated frozen or
concurrency gates.

The `cuda-readback-diagnostic/` tree contains native 24-tick phase timing,
equal four-draw worker-one/worker-four Nsight Systems traces, machine-derived
D2H and per-kernel duration analysis, and bounded Nsight Compute reports for
three evidence-selected kernels. Systems timings decide contention; Compute
reports occupancy, bandwidth, and stalls only because replay destroys overlap.
