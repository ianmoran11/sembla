# Focused H100 final-state A/B/C decision analysis

## Outcome

**B (`packed-pageable`) is eligible for a later production-promotion PRD.**
C (`packed-pinned`) is not eligible.

The frozen protocol completed all 27 executions at repository commit
`7b0e83e0b4284654c3fe1c73382fd939b0317616`. Every A/B/C complete-tree
comparison was byte-identical, every `final_state_sha256` matched, both
independent and CRN noise passed, and the deliberate one-byte negative control
was rejected.

The remote wrapper returned 1 after the protocol because the analyzer rejected
Nsight 2024.6.2's simultaneous `Ctx` and `GreenCtx` columns as ambiguous. The
protocol, all three `.nsys-rep` files, and all CSV exports had already completed.
Local analysis required two schema-compatibility fixes only: prefer an exact
`Ctx` alias over `GreenCtx`, and accept Nsight's `0.000 MB` rounded display for
tiny D2H rows. Sixty-four focused tests pass after those fixes. The derived
`decision.json` and `decision.md` were generated from the unchanged checksummed
protocol; `SHA256SUMS.local-analysis` covers the locally derived files. The
original `SHA256SUMS` still verifies all 658 entries with zero missing or
mismatched files.

## Frozen wall-time gates

The gate uses the median of adjacent within-repetition ratios.

| comparison | workers | median ratio | effect | threshold | result |
|---|---:|---:|---:|---:|---|
| B / A | 1 | 0.847878 | **15.21% faster** | <= 1.02 | pass |
| B / A | 4 | 0.926732 | **7.33% faster** | <= 0.95 | pass |
| C / B | 1 | 1.806184 | **80.62% slower** | <= 1.02 | fail |
| C / B | 4 | 1.209877 | **20.99% slower** | <= 0.95 | fail |

One workers-4 B repetition was slower than its adjacent A control because its
setup phase rose to 4.169 s; the other two ratios were 0.927 and 0.903. The
predeclared median remains 0.927 and passes, but the variability should remain
visible in any promotion decision.

## Why B wins

Across the four-draw timed arms, A's final-state seam was approximately
4.0–4.8 s. B reduced it to approximately 2.0–2.7 s by avoiding host
`StateStore` reconstruction and hashing the packed downloaded byte stream
through the existing canonical framing. The D2H bytes and pageable destination
are unchanged.

Representative workers-4 profile:

| mode | final-state seam | pageable D2H API | reconstruction | CPU SHA-256 |
|---|---:|---:|---:|---:|
| A | 4,785 ms | 1,711 ms | 436 ms | 2,537 ms |
| B | 2,298 ms | 1,246 ms | 0 ms | 1,052 ms |

Nsight attributed four 480 MB final-state D2H copies to both modes. Exposed D2H
was 940.5 ms for A and 923.9 ms for B, confirming that B's gain is host
materialization/hashing rather than a transfer claim.

## Why C loses

Pinned transfer itself worked: Nsight measured the four final-state copies at
46.0 ms exposed versus 923.9 ms for B, and the host-blocking median ratio was
0.203. Both mechanism gates passed.

C nevertheless failed whole-wall gates because cudarc 0.17.6's pinned allocation
is write-combined, requiring a copy into cacheable staging memory before CPU
SHA-256. That staging copy cost approximately 8.0–8.8 s across four draws,
overwhelming the transfer saving. Four workers retained 1.92 GB pinned plus
1.92 GB cacheable staging memory and peaked around 12.27 GB RSS. This closes the
implemented staged pinned route; it does not make a universal claim about every
possible cacheable pinned allocator.

## Decision

1. Draft a small promotion PRD for B: make exact packed-pageable final hashing
   the ordinary CUDA sweep path, preserve canonical SHA-256 and all output
   bytes, retain the diagnostic timing, and remove or demote the experimental
   selector appropriately.
2. Do not promote C and do not add copy streams, double buffering, or more
   pinned staging work without a new measured design.
3. Preserve the failed first profile attempt separately; it found the missing
   Nsight output-parent bug and was fully torn down.

## Safety and teardown

The paid VM and SSH rule were destroyed automatically. Terraform state is empty,
report-only reconciliation shows zero VMs/orphans, teardown status is 0, and GPU
performance-counter access remained admin-only. The independent destroy
watchdog was disarmed after confirming zero resources. No billing resource
remains.
