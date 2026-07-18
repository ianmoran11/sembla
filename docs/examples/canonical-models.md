# Canonical finite-state dynamical systems

These examples exercise dynamical systems that fit Sembla's current fast path:
finite row state, continuous-time hazard specifications, declaration-ordered
transitions, and optional group aggregates. Each model is authored in Lean,
checked in as canonical JSON, validated by Rust, and run twice in
`frontend/scripts/check-parity.sh` to verify byte-for-byte determinism.

The formulas below are the canonical hazards of the corresponding continuous-
time jump processes. The current runtime evaluates them with snapshot-isolated,
fixed-`dt` tau-leaping: guards, aggregates, and rates are frozen at tick start,
and a row that enters a new state cannot transition from that state again in
the same tick. Paths and residence times are therefore tick-discretized rather
than exact continuous event histories. The approximation converges toward the
corresponding continuous-time process as `dt` decreases.

| Example | State | Hazards at the default parameters | Demonstrates |
| --- | --- | --- | --- |
| [`reversible_ctmc.json`](../../examples/reversible_ctmc.json) | `A ↔ B` | `A→B: 0.4`, `B→A: 0.2` | Reversible two-state CTMC / random telegraph process |
| [`radioactive_decay_chain.json`](../../examples/radioactive_decay_chain.json) | `Parent → Daughter → Stable` | `0.25`, then `0.08` | Sequential exponential decay / Bateman chain |
| [`sis_importation.json`](../../examples/sis_importation.json) | `S ↔ I` | infection `0.02 + 0.7 I_c/N_c`; recovery `0.2` | Frequency-dependent SIS with exogenous importation |
| [`seirs_waning.json`](../../examples/seirs_waning.json) | `S → E → I → R → S` | exposure `0.01 + 0.8 I_c/N_c`; progression `0.25`; recovery `0.1`; waning `0.02` | Latency and waning immunity |
| [`noisy_voter.json`](../../examples/noisy_voter.json) | `A ↔ B` | `A→B: 0.02 + 0.8 B_c/N_c`; symmetric reverse rate | Mean-field noisy voter dynamics |

All numeric parameters are symbolic model parameters with LogNormal priors.
The table lists their defaults, not values inlined into the IR.

## Build and validate

From the repository root:

```sh
cargo build --release -p sembla-cli
./target/release/sembla validate examples/reversible_ctmc.json
./target/release/sembla validate examples/radioactive_decay_chain.json
./target/release/sembla validate examples/sis_importation.json
./target/release/sembla validate examples/seirs_waning.json
./target/release/sembla validate examples/noisy_voter.json
```

To export fresh JSON from the Lean definitions:

```sh
cd frontend
lake build
lake exe sembla-export reversible_ctmc /tmp/reversible_ctmc.json
lake exe sembla-export radioactive_decay_chain /tmp/radioactive_decay_chain.json
lake exe sembla-export sis_importation /tmp/sis_importation.json
lake exe sembla-export seirs_waning /tmp/seirs_waning.json
lake exe sembla-export noisy_voter /tmp/noisy_voter.json
cd ..
./target/release/sembla diff-ir examples/reversible_ctmc.json /tmp/reversible_ctmc.json
```

The exporter also accepts camel-case names such as `reversibleCtmc` and
qualified names such as `Sembla.Models.reversibleCtmc`.

## Run the examples

Numeric `--population` initialization is sufficient for every example:

```sh
./target/release/sembla run examples/reversible_ctmc.json \
  --population 1000 --seed 55 --ticks 20 --out reversible_ctmc.csv

./target/release/sembla run examples/radioactive_decay_chain.json \
  --population 1000 --seed 55 --ticks 30 --out radioactive_decay_chain.csv

./target/release/sembla run examples/sis_importation.json \
  --population 1000 --seed 55 --ticks 30 --out sis_importation.csv

./target/release/sembla run examples/seirs_waning.json \
  --population 1000 --seed 55 --ticks 40 --out seirs_waning.csv

./target/release/sembla run examples/noisy_voter.json \
  --population 1000 --seed 55 --ticks 30 --out noisy_voter.csv
```

Repeat a command with a different output name and compare both stdout and CSV
bytes to verify determinism:

```sh
./target/release/sembla run examples/noisy_voter.json --population 1000 --seed 55 --ticks 30 --out first.csv > first.hashes
./target/release/sembla run examples/noisy_voter.json --population 1000 --seed 55 --ticks 30 --out second.csv > second.hashes
cmp first.csv second.csv
cmp first.hashes second.hashes
```

## Generic no-views CSV contract

Models with no declared views use the model-agnostic generic result schema;
this is the default for every canonical model above, not a shape-based special
case. The two comment headers remain canonical:

```text
# params={...}
# dt=...
```

The CSV columns then appear in deterministic declaration order:

1. `tick`;
2. one `count:<box>.<table>.<attribute>=<variant>` column for every variant of
   every enum attribute, iterating boxes, tables, attributes, and variants in
   source order;
3. one `fired:<box>.<transition>` column for every transition in model-global
   rule-ID order, including transitions that fired zero times;
4. `deferred_total`.

Generated header fields are CSV-escaped. For example, the reversible CTMC
starts with:

```text
tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total
```

Counts are observed after each completed tick. For each enum attribute, its
variant columns sum to that table's row count. Every `--out` run also writes an
`<out>.summaries.csv` file; it contains only the `name,value` header for these
views-free, summary-free models. The observation hash of those exact bytes is
printed and recorded in the run manifest. The standalone SIR model instead
declares views and retains its original `tick,S,I,R,...` CSV bytes and hash.
The composed SIR-policy model reports the same views plus firing columns for
all four model-global rules, including `fired_restrict` and `fired_reopen`, as
required by the generic per-rule contract.

Sweeps use whichever columns the model reports. For these views-free examples,
`draw_<k>.csv` keeps the generic schema and `summary.csv` contains deterministic
5/25/50/75/95 bands for each state-count, firing, and deferred column.

## Numeric initialization semantics

For a standalone model, `--population N` initializes every declared table with
`N` rows. Every enum column starts at variant index zero, real and integer
columns start at zero, and reference columns point to row zero of their target
table.

Consequences for these examples:

- the CTMC begins entirely in `A`;
- the decay chain begins entirely in `Parent`;
- SIS and SEIRS begin entirely in `S`;
- the voter model begins entirely in opinion `A`;
- all people/agents in aggregate examples initially reference community row
  zero, so the command-line smoke run is one well-mixed community (other
  community rows are unreferenced).

The positive `import_rate` and `mutation_rate` defaults are deliberate. They
make infection/opinion-B reachable from homogeneous initialization without a
custom population format. They are part of the stated models, not hidden
initial-condition machinery.

## Mathematical definitions

### Reversible two-state CTMC

Each row independently follows

```text
A --rate_ab--> B
B --rate_ba--> A
```

These rates specify the generator of the standard two-state continuous-time
Markov chain, also called a random telegraph process. Sembla converts each
frozen tick hazard to a firing probability and advances the rows on the tick
grid; it does not sample and order exact event times within a tick.

### Radioactive decay chain

Each atom follows

```text
Parent --lambda_parent--> Daughter --lambda_daughter--> Stable
```

`Stable` has no outgoing transition. These are the hazards of the sequential
decay process whose population mean is described by the Bateman equations.
Under the current tau-leap executor, an atom that enters `Daughter` during one
tick cannot decay to `Stable` until a later tick.

### SIS with importation

For a susceptible person in community `c`,

```text
lambda(S→I) = import_rate + beta * I_c / N_c
lambda(I→S) = gamma
```

Importation prevents the disease-free state from being absorbing. `countBy`
and `sizeBy` build `I_c` and `N_c` once per tick and broadcast them through
the community reference.

### SEIRS with waning immunity

```text
lambda(S→E) = import_rate + beta * I_c / N_c
lambda(E→I) = sigma
lambda(I→R) = gamma
lambda(R→S) = omega
```

The hazard specification is the Markovian SEIRS jump process, but the executor
observes exposed, infectious, and immune residence times on the tick grid. For
example, a row that enters `E` during one tick cannot progress to `I` until a
later tick. This is a tau-leaped individual-state process, not exact event-time
simulation or direct ODE integration.

### Noisy voter model

For an agent in community `c`,

```text
lambda(A→B) = mutation_rate + imitation_rate * B_c / N_c
lambda(B→A) = mutation_rate + imitation_rate * A_c / N_c
```

The spontaneous term is the "noise" or idiosyncratic switching rate; the
aggregate term is mean-field imitation of the opposite opinion.

Terminology follows standard descriptions of
[continuous-time Markov chains](https://en.wikipedia.org/wiki/Continuous-time_Markov_chain),
[Bateman decay equations](https://en.wikipedia.org/wiki/Bateman_equation), and
the [noisy voter model](https://arxiv.org/abs/1408.5122).

## Current boundaries

These examples are intentionally limited to what the current IR and runtime
mean precisely. Sembla v0.1 does **not** currently provide:

- continuous-valued ODE/PDE state updates or numerical integrator blocks;
- births, deaths, or changing table row counts;
- one reaction that atomically changes multiple entity rows;
- arbitrary initial enum distributions through numeric `--population`;
- explicit graph-neighbour voter interactions under numeric initialization;
- generic prior-predictive sweep summaries (`sembla sweep` remains SIR-only).

Use `sembla run` for these generic finite-state models. A non-SIR `sweep`
request fails before creating outputs with a clear SIR-only summary error.
