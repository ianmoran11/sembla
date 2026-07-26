# Owned Real numeric-coercion evidence

This directory records the PRD 0004 before/after measurement collected in-run
on 2026-07-26. The fixed case uses the no-grouped demographic model, 1,000,000
slots, 24 ticks, seed 9009, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. The resized model
and initial state were shared byte-for-byte across every execution.

The amended protocol makes quiescence advisory rather than an entry gate.
Measurement therefore proceeded with the PRD runner active and handles noise
by recording `wall - (user + sys)` for every run. A value above 0.5 seconds is
marked contended. Five runs were collected on each side.

## Invocation

Each timed run used:

```text
/usr/bin/time -lp <binary> run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <run-output.csv>
```

Immutable input and binary digests:

- baseline commit:
  `3866d0a7ef5ede54b58cd5e79502efaad26b7aea`
- implementation commit:
  `cdb91e9aea9aa5924b54911f849748a20d02b917`
- before binary:
  `194f6874e3b9f4a16acc737be751f1211b384755dd2f0cd29aa15a046de27bc2`
- after binary:
  `54191a25ba0b6e5d6e897bddb3defae0c613aa527564e8f7a0a2ba506a7da21a`
- resized model:
  `601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`
- initial state:
  `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`

The prepared after binary is byte-identical to `target/release/sembla` at the
implementation commit.

## Measurements

User time is the load-bearing metric, so the headline chooses the uncontended
run with the lowest user time on each side. Wall time is secondary.

| Side | Run | Wall (s) | User (s) | Sys (s) | Wall - user - sys (s) | Contended |
|---|---:|---:|---:|---:|---:|:---:|
| Before | 1 | 7.89 | 5.93 | 1.59 | 0.37 | no |
| Before | 2 | 7.58 | 5.91 | 1.47 | 0.20 | no |
| Before | 3 | 7.89 | 5.95 | 1.67 | 0.27 | no |
| Before | 4 | 7.64 | **5.90** | 1.51 | 0.23 | no |
| Before | 5 | 7.63 | 5.95 | 1.53 | 0.15 | no |
| After | 1 | 8.54 | 6.05 | 1.74 | 0.75 | **yes** |
| After | 2 | 7.69 | **5.93** | 1.55 | 0.21 | no |
| After | 3 | 7.93 | 6.03 | 1.63 | 0.27 | no |
| After | 4 | 7.51 | 5.94 | 1.47 | 0.10 | no |
| After | 5 | 8.08 | 6.09 | 1.63 | 0.36 | no |

Headline fastest-uncontended results by the primary metric:

- before: run 4, **5.90 s user**, 7.64 s wall
- after: run 2, **5.93 s user**, 7.69 s wall

The headline result is 0.9949x in user time, or 0.51% slower after the change.
Wall time is 0.9935x, or 0.65% slower. This is a negligible, slightly negative
result, which PRD 0004 explicitly permits.

Medians across all five runs:

- before: 5.93 s user, 7.64 s wall, 1.53 s system
- after: 6.03 s user, 7.93 s wall, 1.63 s system

After run 1 is the only contended run. The fastest wall-clock runs, before run
2 at 7.58 seconds and after run 4 at 7.51 seconds, do not replace the headline
because user time is primary.

## Output identity

All ten timed outputs and the profiled output are byte-identical:

- primary CSV SHA-256:
  `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`
- summary CSV SHA-256:
  `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`
- manifest SHA-256:
  `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`
- emitted `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`

The manifest comparison covers the complete files, not only the final digest.
The baseline-to-implementation diff does not touch `examples/**`, fixtures,
CSV or hash goldens, Cargo files, IR, CLI, CUDA, or tracked CUDA differential
evidence.

## Full-duration post-change profile

`post-change-full-duration.sample.txt` was captured with `/usr/bin/sample`
started before the benchmark process appeared:

```text
sample sembla-prd0004-profile 600 1 -wait -mayDie \
  -file post-change-full-duration.sample.txt &
sembla-prd0004-profile run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <profile-output.csv>
```

The capture contains 10 startup main-thread samples followed by 6,088 samples
on the process main thread. It includes `execute_tick`, `eval_expr`,
`eval_arithmetic`, `eval_equality`, and `eval_ordering`, confirming that it
covers the evaluator's full execution rather than a fixed prefix. Its SHA-256
is `951da9190eacdbdb07981a3c64093b66a4ab54cee774f22b5c0867993e896350`.

Machine-readable timings, contention flags, digests, and profile metadata are
in `measurements.json`.
