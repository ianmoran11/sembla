# Lock-free CUDA validation first-failure selection

Local implementation evidence for
`docs/prds-cuda-validation-parallelism/0008-remove-the-validation-spin-lock.md`.
Hardware criteria remain pending under `DECISIONS.md` §J14.2.

## Selection rule and payload pairing

Validation still selects the lexicographically smallest full-width
`(scan, order_identity, branch)` key. The three components cannot be packed
losslessly into one 64-bit word, so the implementation adapts the existing
segmented conflict argmin into four stream-ordered launches of each logical
validator:

1. every failure applies `atomicMin` to `scan`;
2. failures matching that scan apply `atomicMin` to `order_identity`;
3. failures matching that prefix apply `atomicMin` to `branch`;
4. only failures matching the complete winning key write the code, reported
   identity, and two detail words.

A CUDA kernel boundary separates every pass and therefore supplies the
whole-device synchronization that cannot be obtained inside one launch. The
single-thread commit kernel runs after payload recovery, publishes
`status[1..=3]` before `status[0]`, and resets scratch for the next validator.
There is no critical section and no retrying CAS over `status[4]`.

This is equivalent to the former selection because each pass restricts itself
to the exact prefix selected by all preceding passes. An exact key identifies
one logical validation check. The only call sites that can be observed more
than once for the same exact key are scalar checks evaluated by multiple
workers; those observations carry identical payload. Consequently concurrent
stores in the recovery pass cannot mix fields from different failures.

`sembla_prepare_effects` is the one replayed validator with side effects. Its
first pass retains the existing owner/value writes. Later passes suppress those
writes and reconstruct only genuine double-write reports from the stable prior
rule owner plus whether an earlier effect in the same transition targets the
same attribute. This keeps PRD 0007's parallel kernel while reproducing the
same branch and paired `(owner, prior_rule, current_rule)` payload.

## Regression coverage

- Generated-source tests reject `atomicCAS(status + 4`, the former spin-lock
  explanation, and the retry-loop protocol while retaining the unrelated
  lock-free `sembla_atomic_min_i64`/`sembla_atomic_max_i64` CAS reductions.
- The host mirror executes all four passes with deliberately different
  observation orders and checks the serial `(scan, identity, branch)` answer
  with its matching code.
- `GEOMETRIES` is now `[(1, 1), (1, 32), (3, 4), (4, 128)]`; the corpus
  `--list` test derives and compares the listed values from that shared
  constant.
- The ignored hardware lib test runs its GPU body in a child process. The
  parent kills and reaps it after a 120-second deadline, turning a device hang
  into a test failure instead of blocking the session indefinitely.
- The checked-in generated SIR source fixture and all existing CSV, hash,
  manifest, state, and differential evidence files remain byte-identical.

## Local checks

The final workspace was checked with:

```text
cargo test --locked
scripts/check-rust.sh
cargo check --locked -p sembla-cuda --features cuda
cargo clippy --locked -p sembla-cuda --features cuda --all-targets -- -D warnings
python3 scripts/check-markdown-links.py
```

The CUDA-feature check and clippy commands compile host-side CUDA integration;
they do not claim GPU execution.

## Hardware criteria pending

No CUDA device is available in the local implementation environment. The
binding hardware command is:

```sh
BENCH_CORPUS=1 bash run-demographic-benchmark.sh
```

Approval under §J14.2 leaves these findings pending:

- `differential-corpus/exit-code.txt` equals zero;
- every negative case reports its expected four-word status under all four
  geometries, including `4x128`;
- total corpus duration is recorded and compared with the 23.04-second
  2026-07-19 baseline.

No wall-time result is claimed by this local correctness implementation.
