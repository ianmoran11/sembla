# CUDA differential harness

Build the CLI with the native CUDA path and compare one model. The single-model
form accepts the run-time `--dt` and `--params` overrides as well as population,
seed, and ticks:

```sh
cargo run --release -p sembla-cli --features cuda -- diff-backends \
  examples/sir.json --population 100000 --seed 77 --ticks 200 \
  --dt 0.25 --params params.json
```

Corpus mode discovers `examples/*.json`, sorts paths bytewise, and runs every
model with the same numeric population, seed, tick count, and optional `--dt`
override. A shared parameter file is rejected because the examples declare
different parameter schemas. Defaults are population 100, seed 1, and 10 ticks;
explicit evidence runs should record all three:

```sh
cargo run --release -p sembla-cli --features cuda -- diff-backends \
  --all-examples --population 100 --seed 7 --ticks 20
```

Composition corpus mode discovers the top-level
`fixtures/plans/*.plan.json` and `fixtures/plans/linked/*.plan.json` files and
sorts their paths bytewise. It includes `two_box.plan.json` and the linked
`epidemic_policy.plan.json`, `two_regions.plan.json`,
`regional_response.plan.json`, and `wrapped_ping_pong.plan.json` fixtures, along
with every other plan in those two directories. Invalid and golden subtrees are
not traversed. Population initialization is unchanged from `run`: composed
models honor each table's nonzero authored `size_hint`, while the supplied
numeric population initializes tables without such a hint. Plans never accept
a `--dt` override. The existing grouped-observation plan remains part of this
exact enumeration and must be rejected; the runner accepts only its exact
`--enable grouped-observations` follow-up diagnostic and treats success or any
other failure as an error. A separate ignored hardware test walks the same two
directories and runs every non-grouped plan individually, so the required
rejection does not prevent supported plan members from receiving differential
coverage.

```sh
cargo run --release -p sembla-cli --features cuda -- diff-backends \
  --all-plan-fixtures --population 1000 --seed 7 --ticks 20
```

## Demographic no-grouped corpus member

The differential runner also admits
`fixtures/demographic/benchmark/demographic_slots.no-grouped.json` explicitly at
a reduced numeric population of 1,000, seed 7, and 20 ticks. This direct
single-model admission leaves both `--all-examples` and `--all-plan-fixtures`
with their existing meanings. The grouped configuration remains CPU-only; a
differential request for it is rejected deterministically with the diagnostic
naming `--enable grouped-observations` and the grouped-observations backend
follow-up.

Before this member was added, the CUDA differential corpus contained no model
that exercised contests or `Ref` dereferences. That coverage gap allowed the
12.3× regression on the demographic path to go undetected while every existing
differential test passed. Differential correctness testing proves that CPU and
CUDA produce the same state and observations for the cases it contains; it does
not establish that the CUDA path is usable at production scale. Correctness and
usability testing are separate obligations.

The runner compares the demographic model under the same exact state-hash,
results-CSV, and summaries-CSV contract as every other differential member and
records its verdict in `demographic-corpus.log`. Hardware execution remains
pending under DECISIONS.md §J14.2 until the strict runner is executed on a CUDA
host. List this member and its frozen corpus parameters without probing
hardware:

```sh
crates/sembla-cuda/scripts/run-differential-corpus.sh --list
```

## Negative validation-diagnostic corpus

PRD 0003 adds four model fixtures whose deterministic eight-row initial state
is defined by the shared test helper in
`crates/sembla-cuda/tests/support/diagnostic_cases.rs`. Each case has bad source
rows `[2, 5, 7]`; the CPU oracle must reject at row 2. CPU `TickError` does not
contain CUDA status words, so the helper separately freezes the normalized CUDA
code and identity:

| Fixture | Expected CUDA status |
| --- | --- |
| `fixtures/validation-negative/claim_key_overflow.json` | `(10, 2)` |
| `fixtures/validation-negative/transition_guard_overflow.json` | `(3, 2)` |
| `fixtures/validation-negative/effect_int_overflow.json` | `(5, 2)` |
| `fixtures/validation-negative/output_expression_overflow.json` | `(9, 1)` |

The first three identities are candidate indices. Output code 9 preserves the
existing target-field identity: field 0 is deliberately safe and field 1 has
the failing expression. Its CPU earliest-row assertion remains separate.
Stored out-of-range Refs are not a case because validated state construction
rejects them before execution and `validate_claims` has no such diagnostic.

Local CPU assertions run in ordinary `cargo test --locked`. The ignored CUDA
unit hardware test uses a private PRD-0002 test seam to download raw status and
runs each case with fresh backends under `1x1`, `1x32`, and `3x4` validation
launches. Hardware execution remains pending until captured on a CUDA host.
List the exact cases and geometries without a GPU or clean-tree requirement:

```sh
crates/sembla-cuda/scripts/run-differential-corpus.sh --list
```

An ordinary GPU-less invocation prints a named `SKIP` reason and exits zero.
Evidence automation must set `SEMBLA_REQUIRE_CUDA=1`, which turns missing CUDA
prerequisites into an error and retains the clean committed-worktree gate:

```sh
SEMBLA_REQUIRE_CUDA=1 \
  crates/sembla-cuda/scripts/run-differential-corpus.sh
```

For each model or plan the command compares every committed state hash, the
exact results CSV bytes, and the exact summaries CSV bytes. It exits at the
first mismatch and reports the tick and CPU/CUDA hash pair. Successful lines
include informational ticks/second for both backends. The CUDA rate times
execution plus the per-tick downloads and read-only formatting required by this
differential mode; it is not a `FinalOnly` production-throughput claim.

Per DECISIONS.md §J14, CUDA uses the content-addressed `rule_word` wherever a
rule identity enters Philox or the deterministic ordering/tie-break key, while
the dense `rule_id` ordinal remains the indexing, layout, specialization, and
diagnostic coordinate. CUDA consumes the words already stamped on the
validated plan model and never recomputes them. Existing legacy evidence
remains valid unchanged because legacy models have `rule_word == rule_id`.

CUDA owns scheduling, conflict resolution, effects, wires, and the evolving
state. The CLI downloads the committed post-tick snapshot only for canonical
hashing and read-only view/result formatting; this observation bridge never
calls the CPU tick executor and is not an execution fallback. CUDA manifests
record the device name and the CUDA Driver API compatibility version returned
by `cuDriverGetVersion` as `gpu_model` and `driver_version`.

Correctness CI may run on any CUDA-capable NVIDIA GPU because native `f64`
produces the selected exact semantics regardless of FP64 throughput.
Performance statements are made only from verified full-rate hardware. This
does not weaken ADR 0001's requirement that production hardware provide
full-rate FP64.

Use `SEMBLA_REQUIRE_CUDA=1 crates/sembla-cuda/scripts/run-differential-corpus.sh`
on a clean, committed remote checkout. On independently verified full-rate hardware, set
`SEMBLA_RUN_FULL_RATE=1`; the runner additionally generates the selected
26,000,000-person / 1,300,000-employer SIR workload shape, executes it for one
tick, and captures its informational rate beside ADR 0001's 1,380.5 ticks/sec
reference. Failure of this optional measurement is recorded but is not a
correctness gate. Provisioning, provenance capture, and teardown
follow `spikes/precision/infra-hyperstack/README.md`. The runner writes a test
log and hardware/driver provenance; copy the verdict and informational
throughput into the dated evidence note before destroying the host.
