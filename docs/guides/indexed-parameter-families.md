# Indexed parameter families

Indexed families make finite demographic parameter tables readable while still
producing Sembla's ordinary scalar model format. They remain supported alongside
the named-domain call notation documented in the
[mathematical model surface guide](mathematical-model-surface.md).

## Authoring

Declare each finite dimension once:

```lean
index age := 0 .. 120
index sex := {male, female}
index location := {nsw, vic, qld}
```

Then declare every parameter cell inline or in a pinned file. Real-family cells
may carry a `LogNormal` prior:

```lean
param β[age, sex] : ℝ where
  [0, male] := 0.10 ~ LogNormal -2.302585092994046 0.25
  [0, female] := 0.09
  -- every remaining combination is required
```

Integer families are priorless and require integer-literal defaults:

```lean
param visits[age] : Int where
  [0] := 0
  -- ...
```

A prior on any `Int` family cell is rejected, whether the cell is inline or
loaded from CSV or JSON.

External tables use a source-relative path and an exact-byte pin:

```lean
param β[age, sex, location] : ℝ
  from csv "data/beta.csv"
  sha256 "<sha256 of exact file bytes>"
```

Use the family in a reaction or in the indexed command-general form:

```lean
infect[age, sex, location] on Person : health: S →[
  β[age, sex, location] · freq (health = I) over employer
] I

transition expose[age, sex, location] on Person where
  guard health = S
  hazard β[age, sex, location]
  set health := I
```

The selected `Person` system must have same-named compatible attributes. The
compiler adds guards and emits one scalar parameter and transition per finite
combination. No tensor lookup reaches Rust or CUDA.

Expansion follows canonical Cartesian order: the leftmost dimension varies
slowest. For `age := 0 .. 1` and `sex := {male, female}`, `β[age, sex]` flattens
in this exact order to `beta_0_male`, `beta_0_female`, `beta_1_male`, and
`beta_1_female` (with transitions named the same way from their own base name).

## Choosing exact ages or age bands

An inclusive integer range is useful when evidence genuinely provides one value
per exact age. It can expand quickly: `121 × 2 × 8` produces 1,936 cells and
transitions. Every parameter or transition family has a default expansion cap
of 10,000. Raise it deliberately for a reviewed model, for example:

```lean
set_option sembla.maxFamilyExpansion 20000 in
sembla_model LargeReviewedModel (dt := 1.0) where
  -- declarations
```

Prefer an enum age-band index when source data are banded. An integer index range
does not constrain runtime state: rows outside the range simply match none of
that transition family's generated guards.

## CSV format

The dimension columns must match the family declaration exactly and are followed
by four fixed columns:

```csv
age,sex,location,default,prior_family,prior_arg_1,prior_arg_2
0,male,nsw,0.10,log_normal,-2.302585092994046,2.5e-1
0,female,nsw,0.09,,,
```

Rows may be reordered. Missing, duplicate, extra or out-of-domain cells fail.
The original unversioned external CSV form is CSV v1 and accepts only
`log_normal`. To use exactly `normal` and `log_normal`, select CSV v2 explicitly:

```lean
param β[age, sex, location] : ℝ
  from csv "data/beta-v2.csv" schema "sembla.parameter-family/v2"
  sha256 "<sha256 of exact file bytes>"
```

CSV supports standard quoted fields and LF or CRLF line endings. Decimal text,
including trailing zeros, long decimals, and scientific notation, is retained
without semantic conversion through `Float`.

## JSON format

```json
{
  "schema_version": "sembla.parameter-family/v1",
  "dimensions": ["age", "sex", "location"],
  "cells": [
    {
      "key": {"age": 0, "sex": "male", "location": "nsw"},
      "default": "0.10",
      "prior": {"family": "log_normal", "args": ["-2.302585092994046", "2.5e-1"]}
    }
  ]
}
```

Defaults and prior arguments must be JSON strings. JSON v1 accepts only the
`log_normal` prior family. JSON v2 selects mixed-prior support inside the file
(no Lean-side `schema` clause is used for JSON):

```json
{
  "schema_version": "sembla.parameter-family/v2",
  "dimensions": ["age", "sex", "location"],
  "cells": [
    {
      "key": {"age": 0, "sex": "male", "location": "nsw"},
      "default": "0.0",
      "prior": {"family": "normal", "args": ["0.0", "4.166666666666667e-06"]}
    },
    {
      "key": {"age": 0, "sex": "female", "location": "nsw"},
      "default": "0.09",
      "prior": {"family": "log_normal", "args": ["-2.407945608651872", "0.5"]}
    }
  ]
}
```

For both versions, `prior` may be null, every Cartesian cell is mandatory, and
the decoder rejects unknown fields and schema versions. V2 permits exactly
`normal` and `log_normal`; it does not add other IR prior families.

## Hashing, paths, and regeneration

Paths must be relative (absolute paths are rejected) and are resolved relative
to the Lean source containing the declaration. They are hash-pinned but are
**not** sandbox-confined: `..` paths and symlinks can escape the source
directory. Imported Lean source and its external tables are trusted under the
same build trust boundary.

Compute a pin with:

```sh
shasum -a 256 path/to/table.csv
```

The compiler reads bytes and checks SHA-256 before UTF-8 or table parsing, so a
wrong pin on malformed data reports the hash mismatch first. A generator should
write the complete table deterministically, compute the exact byte hash, and
update the Lean hash literal in the same change.

Because Lean 4.13 does not track arbitrary data files for incremental Lake
builds, changing only table bytes can leave a stale `.olean`; changing the hash
literal forces the declaring module to rebuild. CI should always elaborate the
external-table test module directly.

Run:

```sh
cd ../sembla-lean
lake env lean Sembla/ParameterTableTests.lean
lake env lean Sembla/IndexedFamilyTests.lean
bash scripts/test-negative.sh
```

## Australian production migration

The Australian population model now uses the named-domain mathematical surface
in
[`Surface.lean`](https://github.com/ianmoran11/sembla-lean/blob/main/Sembla/Models/AustralianPopulation/Surface.lean)
and four source-relative, SHA-pinned JSON v2 tables under
`AustralianPopulation/Data/`. Those tables provide 8 birth, 336 mortality, 8
arrival, and 8 emigration cells; the mortality table mixes seven centred
`Normal` zero-rate exceptions with `LogNormal` positive-rate priors. Static
lowering retains the established 377 scalar parameters and 418 scalar
transitions.

`Surface.lean` plus those four pinned v2 tables is production authority.
`Parameters.lean` and `Transitions.lean` are compatibility projections. The
generated
`Sembla/TestData/AustralianPopulation/AustralianPopulationParameters.lean` in
`sembla-lean` is reproducible
377-parameter structural evidence only, while large generated state artifacts
remain ignored under `data/abs/generated/`.

See the [decision record](../prds-indexed-families/README.md) for frozen naming,
ordering and compatibility details and the
[mathematical guide](mathematical-model-surface.md) for the superseding
named-domain authoring surface.
