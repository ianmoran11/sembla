# Workplace SIR example

`examples/sir.json` is the v0.1 end-to-end epidemic model. Its `sir.person`
table has `health: Enum{S,I,R}` and `employer: Ref{employer}`; `sir.employer`
is the group domain. Both rates remain symbolic parameters with log-normal
prior metadata.

## Hazard formula

For susceptible person `p` in workplace `w`, the checked-in expression is the
standard frequency-dependent hazard

```text
lambda_infect(p) = beta * I_w / N_w
lambda_recover(p) = gamma
```

`I_w` and `N_w` are both PRD 0005 group-by `Count` accumulators over `person`,
joined by `employer`. They are built once per tick and broadcast to people;
they are not recomputed per person. In the default model `beta=0.8`,
`gamma=0.1`, and therefore the homogeneous approximation is `R0=beta/gamma=8`.
The model tick is `dt=0.25`; `--dt` explicitly overrides it for a run.

## Deterministic synthetic population

Generate one million people, 50,000 employers, and exactly 100 initial
infections:

```sh
cargo run --release -p sembla-cli -- synth-pop \
  --persons 1000000 --employers 50000 --initial-infected 100 \
  --seed 2025 --out pop.bin
```

Generation uses only PRD 0003 Philox coordinates. Rule ID `0xffff_ff00` is
reserved for workplace assignment and `0xffff_ff01` for the deterministic
initial-infection Fisher-Yates permutation. Workplace assignment is
`floor(E * U^2)`, a documented power-law-ish bucketing that produces many
small and progressively fewer large workplaces. It is deterministic rather
than a demographic claim.

`pop.bin` is a portable, versioned little-endian format: 12-byte
`SEMBLA_POP\0\0` magic, `u32` version, `u64` person and employer counts, then
all `u16` health indices and all `u32` employer references in person-row
order. The loader rejects wrong magic/version, truncation, trailing data,
invalid health indices, and out-of-range employer references.

## Run and inspect results

```sh
cat > params.json <<'JSON'
{"beta":0.8,"gamma":0.1}
JSON
cargo run --release -p sembla-cli -- run examples/sir.json \
  --population pop.bin --seed 99 --ticks 100 --dt 0.25 \
  --params params.json --out results.csv
```

The model declares three filtered count views, `S`, `I`, and `R`, over the
committed post-tick `person.health` state. Those declarations generically
produce the legacy byte-for-byte CSV schema
`tick,S,I,R,fired_infect,fired_recover,deferred_total`; the CLI contains no
model-name or SIR-shape output branch. The final stdout line prints SHA-256
digests of the exact results bytes, final columnar state, and observation
summary bytes. Unknown parameters and values with the wrong declared JSON type
are errors that name the parameter.

The command also writes `results.csv.summaries.csv` in declaration order:

```text
name,value
peak_I,...
peak_tick,...
```

`peak_I` is `max(I)` and `peak_tick` is the earliest tick attaining that
maximum.

The command also writes `results.csv.manifest.json`, a canonical compact JSON
sidecar with sorted keys and one trailing newline. It records schema versions,
the effective canonical-IR hash (including `--dt`), model name, seed, ticks,
`dt`, determinism level `A`, sorted resolved theta, the population basename (or
numeric specification) and input hash, CPU backend/precision/fallback identity,
enabled flags, result, final-state, and observation hashes, hash algorithm IDs
(`sha256`), and workspace component versions. It deliberately contains no timestamp, host, or
absolute path.

## Verify a recorded run

Re-run the recorded contract with the original model and population inputs:

```sh
cargo run --release -p sembla-cli -- verify-run \
  results.csv.manifest.json examples/sir.json \
  --population pop.bin --params params.json
```

A matching execution prints `verified 1 execution(s)` and exits zero. A changed
model or population, or a tampered seed, `dt`, resolved-theta value, result
hash, or final-state hash exits one and prints a field-by-field mismatch.
Manifest readers also reject unsupported schema-version majors and an
incomplete `backend_identity` tuple.

## Verify determinism

The run contract is seed + IR + resolved theta. These two commands must print
the same three hashes and produce byte-identical result CSV, summaries CSV,
and manifest sidecars (apart from
the explicitly recorded output population basename when different population
filenames are used):

```sh
cargo run --release -p sembla-cli -- run examples/sir.json --population pop.bin --seed 99 --ticks 100 --params params.json --out first.csv
cargo run --release -p sembla-cli -- run examples/sir.json --population pop.bin --seed 99 --ticks 100 --params params.json --out second.csv
```

Changing `--seed` or a value in `params.json` changes the results and final
state hashes and may change the observation hash. The automated integration test performs this check at 100,000
people for 100 ticks.

## Prior-predictive sweep

Draw 20 parameter vectors from the priors declared in `examples/sir.json`
and run the same population for 50 ticks under independent simulation noise:

```sh
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --draws 20 --ticks 50 \
  --noise independent --out sweep/
```

`--noise crn` is the default and preserves the historical sweep bytes: every
θ draw reuses the master simulation seed, which is useful for paired policy or
sensitivity contrasts. Use `--noise independent` for NPE training data; it
derives a stable simulation seed from the master seed and replica index for
each draw without changing θ. As explained in `DECISIONS.md` §G5, CRN is wrong
for training pairs because one shared noise realization teaches an artificially
deterministic θ→x mapping and produces an overconfident learned posterior.

The directory contains `manifest.csv` (the pre-existing theta table for each
draw), one standard `draw_<k>.csv` result per draw, `summary.csv`, and
`run-manifest.json`. These two manifest names are intentionally distinct:
`manifest.csv` is only a tabular parameter report, while `run-manifest.json`
is the canonical reproducibility contract. The JSON stores shared model,
population, seed, tick, backend, schema, and component fields once, then one
`executions` entry per draw with `k`, its actual simulation seed, sorted
resolved theta, results hash, and final-state hash. It also records the
`noise_mode` and the all-or-nothing `theta_source` kind/hash/algorithm tuple.
The summary reports the nearest-index 5/25/50/75/95 percentiles for every
reported per-tick column—here S, I, R, transition firings, and deferred events.
The same sweep command also works for views-free models using their generic
state-count/firing columns. Stdout prints SHA-256 digests for the CSV parameter
manifest and summary. In prior mode, `theta_source.sha256` is the effective
canonical-IR digest because that IR contains the prior declarations.

Verify every recorded draw, or select one draw with `--draw`:

```sh
cargo run --release -p sembla-cli -- verify-run \
  sweep/run-manifest.json examples/sir.json --population pop.bin
cargo run --release -p sembla-cli -- verify-run \
  sweep/run-manifest.json examples/sir.json --population pop.bin --draw 3
```

Pin any subset of parameters with the ordinary override format. Pinned values
are marked in the manifest header and are not sampled:

```sh
printf '{"gamma":0.1}\n' > pinned.json
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --draws 20 --ticks 50 \
  --params pinned.json --out sweep-pinned/
```

External sequential or proposal methods can supply an ordered JSON list of θ
objects instead of sampling priors. Every entry must provide every
prior-bearing parameter; the entry count is the draw count, so `--theta-file`
and `--draws` are mutually exclusive:

```sh
printf '[{"beta":0.7,"gamma":0.12},{"beta":0.8,"gamma":0.1}]\n' > theta.json
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --theta-file theta.json --ticks 50 \
  --noise independent --out sweep-proposals/
```

The file-mode `manifest.csv` starts with `# theta_source=file`, the JSON run
manifest records the exact input-file SHA-256, and stdout prints it as
`theta_file_sha256`.

### Exporting `(θ, x)` training pairs

Add `--export-pairs` to write the summaries-only training input for the
PRD-0007 NPE pipeline:

```sh
cargo run --release -p sembla-cli -- sweep examples/sir.json \
  --population pop.bin --seed 99 --draws 5000 --ticks 50 \
  --noise independent --out sweep-training/ \
  --export-pairs pairs.csv
```

`pairs.csv` has one row per draw. Its columns are `k`, parameters sorted by
name (`beta,gamma` here), then summaries in model declaration order
(`peak_I,peak_tick` here). Values use the same deterministic formatting as the
ordinary sweep and summary CSVs. Only declared summaries become `x`; per-tick
view series are not included. CRN export is allowed for diagnostics but emits
a warning because those pairs are unsuitable for NPE training.

The canonical `pairs.csv.meta.json` sidecar binds the bytes to the effective IR,
master seed, noise mode, theta source, draw/tick/`dt` settings, determinism
level, ordered columns, component versions, and `pairs_sha256`. PRD-0007 reads
only this CSV and sidecar, verifies their schema and hash, and rejects CRN-mode
training input. Prior-sampled and `--theta-file` sweeps use the same export
contract.

`sembla compare --out comparison.csv` similarly writes
`comparison.csv.manifest.json`. Its `executions` array contains deterministic
`arm_a` and `arm_b` scenario entries with each arm's model, effective IR hash,
`dt`, resolved theta, results hash, and final-state hash; population, seed,
ticks, backend identity, flags, and component versions remain shared.

Normal priors use the frozen cosine branch of Box--Muller; LogNormal draws are
the exponential of that Normal draw. Parameter coordinates reserve
`rule_id = 0xffffffff`, use the draw index as `tick`, and the parameter's
declaration index as `entity_id`, so extending K never changes an earlier θ
draw. Independent simulation seeds reserve `rule_id = 0xfffffffe`, use the
replica index as `tick`, and set `entity_id = draw_idx = 0`; Philox output lane
0 supplies the low 32 bits and lane 1 the high 32 bits of the derived `u64`.
Thus extending K also never changes an earlier replica seed or result.

## CUDA backend and differential check

Request the native-`f64` backend explicitly; an unavailable device or toolkit
is a nonzero error and never falls back to CPU:

```sh
cargo run --release -p sembla-cli --features cuda -- run \
  examples/sir.json --population pop.bin --seed 77 --ticks 200 \
  --backend cuda --out results-cuda.csv
```

The CUDA sidecar records `backend=cuda-native-f64`, `precision=f64`,
`fell_back=false`, and nonempty `gpu_model` and `driver_version` fields.
`verify-run` reads that identity and replays CUDA rather than substituting the
CPU oracle.

For CPU-oracle differential testing and the hardware policy, see
[`docs/cuda-differential-harness.md`](../cuda-differential-harness.md).
