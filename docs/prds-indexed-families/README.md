# Indexed parameter and transition families

**Status:** Implemented.  
**Scope:** Lean authoring sugar only; the scalar IR, JSON, validators and CPU/CUDA runtimes are unchanged.

> **Superseding decision (later mathematical-surface migration).** This record's
> closing restriction that the Australian population model was “deliberately not
> migrated” described the scope of this earlier indexed-family delivery. It was
> later superseded by the approved mathematical-surface migration. Production now
> uses `AustralianPopulation/Surface.lean` plus four pinned
> `sembla.parameter-family/v2` JSON tables, including mixed `Normal` and
> `LogNormal` mortality priors, while statically preserving the scalar IR. The
> original restriction is retained below as historical context, not current
> maintenance policy. See the
> [mathematical model guide](../guides/mathematical-model-surface.md).

## Decision

Finite model-level domains are declared in source order:

```lean
index age := 0 .. 120
index sex := {male, female}
```

Ranges are inclusive and non-negative. An index is a compile-time expansion
domain, not a runtime bound on an `Int` state column. Enum members are ordered.
The leftmost dimension varies slowest.

A complete inline family is:

```lean
param β[age, sex] : ℝ where
  [0, male] := 0.10 ~ LogNormal (-2.302585092994046) 0.25
  [0, female] := 0.09
```

Every Cartesian cell must occur exactly once; wildcard and fallback cells are
not supported. `Int` families use integer defaults and cannot have priors.
Cells are emitted in canonical Cartesian order regardless of table row order.
`β[0,male]` becomes the ordinary scalar parameter `beta_0_male`.

External tables are source-relative and always pinned over their exact bytes:

```lean
param β[age, sex] : ℝ from csv "data/beta.csv" sha256 "<64 lowercase hex>"
param β[age, sex] : ℝ from json "data/beta.json" sha256 "<64 lowercase hex>"
```

The hash is checked before UTF-8 decoding or schema parsing.

## Static transition expansion

```lean
infect[age, sex] on Person : health: S →[
  β[age, sex] · freq (health = I) over employer
] I
```

Each binder resolves both a declared index and a same-named attribute on the
explicitly selected system. Range indexes require `Int`; enum indexes require
the identical ordered enum domain. The compiler emits `infect_0_male`, etc.,
and adds equality guards after the authored/source-state guard in binder order.
Indexed command-general transitions use the same rules.

An indexed parameter reference must name a declared family, list its dimensions
in declared order, and occur inside a transition instance binding those indexes.

## External schemas

CSV has the exact header:

```text
<dimension columns>,default,prior_family,prior_arg_1,prior_arg_2
```

Prior columns are all empty or `log_normal,<location>,<spread>`. Quoted fields,
doubled quotes, LF and CRLF are supported.

JSON is strict and versioned:

```json
{
  "schema_version": "sembla.parameter-family/v1",
  "dimensions": ["age", "sex"],
  "cells": [{
    "key": {"age": 0, "sex": "male"},
    "default": "0.10",
    "prior": {"family": "log_normal", "args": ["-2.302585092994046", "0.25"]}
  }]
}
```

Defaults and prior arguments are strings so exact decimal spellings reach the
existing `Scientific` path. `prior` may be `null`; unknown fields are rejected.

## Ordering, limits and compatibility

Family parameters and transitions occupy their declaration's source position.
Flattened names participate in ordinary duplicate-name checks, so ambiguous
underscore flattenings fail instead of overwriting. Each declaration is capped
at 10,000 emitted instances by default; authors may deliberately raise
`set_option sembla.maxFamilyExpansion`.

Lean 4.13 does not make arbitrary external files Lake dependencies. Legitimate
data changes must also update the mandatory hash literal, which changes the
Lean source and forces recompilation. Fresh CI and direct elaboration detect a
file changed without its pin.

Exact inline/CSV/JSON/manual twins live in
[`frontend/Sembla/IndexedFamilyTests.lean`](../../frontend/Sembla/IndexedFamilyTests.lean).
The Australian population model is deliberately not migrated by this feature.
