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

The CSV starts with canonical resolved-theta and `dt` comment headers, then
rows with
`tick,S,I,R,fired_infect,fired_recover,deferred_total`. The final stdout line
prints SHA-256 digests of the exact results bytes and final columnar state.
Unknown parameters and values with the wrong declared JSON type are errors
that name the parameter.

## Verify determinism

The run contract is seed + IR + resolved theta. These two commands must print
the same two hashes:

```sh
cargo run --release -p sembla-cli -- run examples/sir.json --population pop.bin --seed 99 --ticks 100 --params params.json --out first.csv
cargo run --release -p sembla-cli -- run examples/sir.json --population pop.bin --seed 99 --ticks 100 --params params.json --out second.csv
```

Changing `--seed` or a value in `params.json` changes the results and final
state hashes. The automated integration test performs this check at 100,000
people for 100 ticks.
