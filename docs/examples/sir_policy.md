# SIR + policy feedback and CRN comparisons

`examples/sir_policy.json` composes the PRD 0008 workplace SIR population with
a one-row policy controller. It is a real two-box feedback loop:

1. after each population tick, `population.infection_count` emits the total
   number infected;
2. that owned one-row table is available to `policy` on the next tick;
3. when infections exceed **500**, an `Open` controller switches to
   `Restricted` and sets its state attribute `modifier` from `1.0` to `0.4`;
4. when infections later fall below **150**, it reopens and restores `1.0`.

The distinct 500/150 thresholds are hysteresis: noise around one threshold
cannot toggle policy every tick. Threshold transitions use the documented
finite hazard `1e300`. Every positive PRD 0003 exponential draw divided by
that rate is below `dt=0.25`, so a true guard fires deterministically without
adding a special IR hazard form.

## Hazard and tick-zero encoding

The population keeps the PRD 0008 frequency-dependent workplace hazard,
multiplied by the restriction:

```text
lambda_infect(p,t) = beta * I_employer(p,t) / N_employer(p) * modifier(t)
```

PRD 0007 inputs are schema-carrying **zero-row tables at tick 0**. An input
sum therefore has additive identity zero, whereas the required tick-zero
contact modifier is one. The wire consequently carries the neutral offset
`modifier_offset = policy.modifier - 1`; the population evaluates
`1 + sum(modifier_offset)`. This is `1 + 0 = 1` for the empty tick-zero input,
then exactly `1.0` or `0.4` after delivery. The actual policy state remains the
specified `modifier: Real` with values `1.0` and `0.4`.

Outputs are Moore-style and wires have the synchronous delay described in
`DESIGN.md` section 10.7. If the policy transition fires on tick `t`, its new
output is installed after that tick and first changes the population hazard on
**tick `t + 1`**. The infection count has the same one-tick delivery rule in
the other direction.

## Generate a shared population

```sh
cargo run --release -p sembla-cli -- synth-pop \
  --persons 100000 --employers 500 --initial-infected 100 \
  --seed 12 --out pop.bin
```

## Policy versus no-policy model contrast

Both arms use seed 55 and the same population bytes:

```sh
cargo run --release -p sembla-cli -- compare \
  examples/sir.json examples/sir_policy.json \
  --population pop.bin --seed 55 --ticks 200 --out policy-compare.csv
```

The CSV records canonical resolved parameter vectors for both arms, followed
by side-by-side `S`, `I`, `R`, fired counts, and signed differences `B - A`.
The standalone SIR model is arm A and the policy model is arm B.

This is a common-random-numbers (CRN) comparison. The population box keeps the
standalone model's `infect` and `recover` declaration order and therefore its
global rule IDs 0 and 1. With a shared seed, matching
`(tick, rule_id, entity_id, draw_idx)` coordinates receive identical Philox
draws. Differences are attributable to the policy intervention rather than to
one arm receiving luckier shocks. The arms are still independent state stores;
comparison does not couple their state or consume a shared mutable RNG.

Run the same command twice with different output names and compare exact bytes:

```sh
cargo run --release -p sembla-cli -- compare examples/sir.json examples/sir_policy.json --population pop.bin --seed 55 --ticks 200 --out first.csv
cargo run --release -p sembla-cli -- compare examples/sir.json examples/sir_policy.json --population pop.bin --seed 55 --ticks 200 --out second.csv
cmp first.csv second.csv
```

## Parameter contrast

A one-model contrast pairs two resolved theta vectors under the same IR and
seed. For example:

```sh
printf '%s\n' '{"beta":0.8,"gamma":0.1}' > params-a.json
printf '%s\n' '{"beta":0.4,"gamma":0.1}' > params-b.json
cargo run --release -p sembla-cli -- compare examples/sir.json \
  --population pop.bin --seed 55 --ticks 200 \
  --params-a params-a.json --params-b params-b.json \
  --out beta-compare.csv
```

Here arm B's lower `beta` produces a lower paired final attack rate. The
resolved theta for each arm is echoed in the header, making the comparison
self-describing.
