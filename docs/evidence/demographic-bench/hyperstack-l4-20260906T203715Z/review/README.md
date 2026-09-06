# Independent review of the completed H100 collection

The measured candidate is `107a686cebbcc4b002aa90b34cbdfb4448405860`; the
baseline is `d93c8a301f1fc6e0e44ec6ed06504c5b45ff363e`. The runtime implementation
is unchanged from `fc01302e3db9f8aa2f874ccf4f38514c1d85b8a7`. The later candidate
fixes diagnostic test selection; both candidate CLI binaries have identical
SHA-256 values.

Three alternating measured CUDA pairs follow a warmup pair at each scale.
`summary.json` excludes warmup 0. Median whole-command times are:

| Slots | Baseline | Candidate | Time reduction | Peak host RSS before → after |
|---|---:|---:|---:|---:|
| 1M | 3.622 s | 3.444 s | 4.9% | 420.99 → 406.82 MiB |
| 10M | 23.669 s | 21.851 s | 7.7% | 2510.98 → 2468.50 MiB |

All six measured pairs improved. Individual 10M reductions range from 2.2% to
8.4%, so host variation remains visible. The gains mainly concern loading and
construction; typical later-draw times are almost unchanged. The timings do not
isolate the contribution of each implementation change.

## Verification

`verify-gpu.py` verified all 3,162 remote checksum entries; exact before/after
CPU and CUDA output trees; all warmup and measured CUDA repeat trees and pair
exports; cross-backend output trees with only backend identity normalized; the
comparator's negative control; and all six frozen-gate scientific outputs,
summaries and hash tuples. The full differential corpus passed. Both diagnostic
logs contain 20 case/geometry results and four large-row recovery results;
Compute Sanitizer ran all four hardware test bodies with zero errors.
`profile-verification.json` additionally records nine exact profile comparisons
and observed validation grid geometry. The frozen §L4 gate is MET at 6.321×.

The interrupted collection at [the incomplete snapshot](../../hyperstack-l4-20260906T202456Z/INCOMPLETE.md) had a diagnostic
child select zero tests. Its parent success was not accepted as complete
correctness evidence. It is retained separately and is not the performance
sample summarized here.

## CPU limitation

The AMD EPYC 9554 host's 1M CPU controls were slower in both collections:
54.79 → 63.04 seconds in the interrupted collection (+15.1%), and
58.04 → 65.60 seconds in this collection (+13.0%). The 10M CPU control was
715.47 → 716.38 seconds (+0.1%). CPU execution source is identical between arms,
but input allocation changed; the cause of the 1M slowdown is unresolved.
These results do not establish CPU neutrality or a CPU throughput improvement.

A subsequent local M2 Pro check used the exact same 1M input digest and five
24-tick draws, with a warmup pair and three alternating measured pairs. Median
whole time was 10.853 → 10.770 seconds (0.8% lower), with exact trees and pair
exports. This did not reproduce the AMD-host slowdown; it does not rule it out.
Commands, raw native timings and outputs are in `cpu-control-followup/`.
A focused AMD-host allocation/cache or NUMA investigation is the next CPU step;
no further paid VM was created during this follow-up.

## Local collection and closeout

The collector transferred and verified the remote archive and destroyed both
paid resources. Its final *local* checksum check then failed because Finder
changed `.DS_Store`. All remote checksum entries remained valid. The two local
manifest generators now exclude this mutable OS metadata; both actual generators
and all 12 collector flag tests pass. Raw remote files and
`SHA256SUMS.remote` are preserved; the final local manifest includes this review
and excludes `.DS_Store`.

`provider-reconciliation.txt` confirms zero VMs. `session-closeout.json` records
watchdog disarm, disposable credential cleanup and console-password deletion.
The apply-request-to-provider-empty interval was 59m 33s: about US$1.99 at the
quoted rate, including provisioning latency, rather than a provider invoice.
