# Australian population Lean modules

This directory implements the unchanged public
`Sembla.Models.australianPopulation` exports.

## Module map

- `Surface.lean` owns the preserved schema-only export and the complete
  declarative model: named domains, projected age bands, scalar parameters,
  pinned parameter functions, aliases, relations, views, and summaries.
- `Data/*.json` are generated complete `sembla.parameter-family/v2` tables for
  births, mortality, overseas arrivals, and emigration.
- `Parameters.lean` and `Transitions.lean` are compatibility projections only.
- `Validation.lean` pins the four table byte hashes and checks schema,
  parameter, transition, public-export, and `checkModel` invariants.
- the sibling `../AustralianPopulation.lean` directly exposes the declarative
  model and its existing model/model-JSON/plan-JSON exports.

## Generated-table contract

`data/abs/rates.py` generates the four tables from the same in-memory 2010
values and prior registry used for `params/2010.json` and `params/priors.json`.
It also retains `data/abs/reference/AustralianPopulationParameters.lean` as
byte-reproducible 377-parameter structural evidence; production never imports
that reference or overwrites the compatibility `Parameters.lean`:

```sh
python3 data/abs/rates.py
```

For isolated generation, redirect all outputs:

```sh
python3 data/abs/rates.py \
  --params-dir "$tmp/params" --report "$tmp/rates.md" \
  --parameter-tables "$tmp/tables" \
  --lean-parameters "$tmp/Parameters.reference.lean"
```

Tables are deterministic UTF-8 with LF endings and a final newline. Their
canonical products contain 8 birth, 336 mortality, 8 arrival, and 8 emigration
cells. The parameter order remains 17 free scalars followed by those 360 cells;
exactly seven mortality cells use `Normal`, with all other priors `LogNormal`.
Any count, order, value, prior, pin, or byte comparison failure is a hard
failure; never refresh the canonical fixtures to hide it.

## Validation

Run from the repository root:

```sh
python3 -m unittest data.abs.tests.test_rates
(cd frontend && lake env lean Sembla/Models/AustralianPopulation/Validation.lean)
./scripts/check-abs-data.sh
frontend/scripts/check-parity.sh
cargo test -p sembla-cli --test australian_population
```

Parity must retain fixture SHA-256 values
`6f307418a367d44d6cde56c18c57d6873981a7187bc17fa94ca3c5f9110ed904`
(model) and
`6f05d870681f35950deab4c74a8a9e816b43625a744f9e8cf2d5aa96a2ef18fe`
(plan).
