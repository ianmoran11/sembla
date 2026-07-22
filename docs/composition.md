# Composition guide

Sembla's first composition release has one executable path:

```text
Lean surface syntax → canonical composition source → Lean linker
  → canonical executable plan → Rust validation → deterministic run
```

The normative rules are in [`DECISIONS.md` §J](../DECISIONS.md#j-composition-and-the-option-d-architecture-accepted-2026-07-21).
The architecture and rollout boundaries are described in
[`docs/design/option-d-architecture.md`](design/option-d-architecture.md).

## Author components and a root composition

`sembla_component` defines an ordinary Lean constant. A primitive component
uses the same `system`, `input`, transition, `output`, and `view` declarations
as `sembla_model`; `requires` names model-level parameters used by its body.
Components do not declare `dt`.

A composite component instantiates component constants and explicitly wires,
exposes, or hides their ports:

```lean
sembla_component EpidemicPolicy where
  instance population : Population with [
    beta := beta,
    gamma := gamma,
    policy_strength := policy_strength
  ]
  instance policy : Policy with [
    threshold := threshold
  ]
  wire count_to_policy : population.infection_count -> policy.infection_count
  wire modifier_to_population : policy.restriction_modifier -> population.restriction_modifier
```

Bindings always map a component requirement to a root model parameter. An
omitted binding is the explicit same-named binding; literals are not accepted.
Wire and exposure declarations require stable labels. `hide` does not.

`sembla_composition` defines the root source. It has exactly one root instance,
an exact lowercase slug name, the outer `dt`, root parameters, and optional
summaries:

```lean
sembla_composition epidemicPolicyModel
    (name := "epidemic_policy") (dt := 0.25) where
  param beta : Real := 0.3
  param gamma : Real := 0.1
  param policy_strength : Real := 0.6
  param threshold : Real := 100.0
  root epidemic : EpidemicPolicy
  summary infected_peak := max epidemic.population.I
```

See
[`frontend/Sembla/Composition/SurfaceModels.lean`](../frontend/Sembla/Composition/SurfaceModels.lean)
for complete primitive, composite, nested, and root examples.

## Export the canonical source

Build the authored module, then export its registered source constant:

```sh
cd frontend
lake build Sembla.Composition.SurfaceModels
lake exe sembla-export --source surface_epidemic_policy \
  ../build/epidemic_policy.source.json
```

The exported bytes use `sembla.composition-source/v1` and
`sembla.canonical-json/v1`. Source arrays preserve author order. The Rust
runtime does not execute composition sources directly.

## Link a source

Single-file mode writes an independently runnable plan and can also write the
non-semantic link report:

```sh
cd frontend
lake exe sembla-link ../build/epidemic_policy.source.json \
  --plan ../build/epidemic_policy.plan.json \
  --report ../build/epidemic_policy.link-report.json
```

Bundle mode creates a new or empty directory and writes the frozen four-file
layout:

```sh
lake exe sembla-link ../build/epidemic_policy.source.json \
  --bundle ../build/epidemic_policy.bundle
```

```text
composition-source.json
executable-plan.json
link-report.json
bundle-manifest.json
```

A non-empty destination is rejected rather than overwritten. The bundle
manifest records source, semantic-plan, envelope-plan, and bundle-root SHA-256
records. A plan never embeds hashes of itself; a linked plan does embed its
source-hash provenance. The report is non-semantic but remains a named input
to bundle integrity. Copying `executable-plan.json` out of the bundle does not
make it incomplete.

Verify all versions, canonical bytes, hashes, bundle membership, plan validity,
and source-plan agreement with:

```sh
cd ..
cargo run -p sembla-cli -- bundle-verify build/epidemic_policy.bundle
```

Relinking old sources and automating historical-linker retention are future
work. Keep the original plan and bundle when a historical result must remain
reproducible.

## Validate and run the plan

```sh
cargo run -p sembla-cli -- validate build/epidemic_policy.plan.json
cargo run -p sembla-cli -- run build/epidemic_policy.plan.json \
  --population 1000 --seed 55 --ticks 40 --out build/results.csv
cargo run -p sembla-cli -- verify-run build/results.csv.manifest.json \
  build/epidemic_policy.plan.json --population 1000
```

Plan envelopes currently run on the deterministic CPU backend. Composition
does not add a second runtime semantics: the linker emits the same flat model
shape consumed by existing validation and execution.

## Identity and refactoring

V1 stable identities use this grammar:

- slug: `[a-z][a-z0-9_]*`;
- declaration: `<kind>:<slug>` for `model`, `def`, `inst`, `port`, `wire`, or
  `expose`;
- occurrence: `occ:` plus the slash-joined instance-ID slugs from the root;
- transition occurrence: `<occurrence>#<transition-name>`;
- wire occurrence: `<owner-occurrence>#wire:<wire-slug>`;
- mailbox: `mbox:<wire-occurrence>|<source-occurrence>.<port>|<target-occurrence>.<port>`;
- plan leaf: the slash-joined occurrence chain without the `occ:` prefix.

Display names are provenance only. Renaming a display label, reordering
independent declarations, or permuting source definitions preserves stable
identity and canonical linked bytes where the fixtures assert permutation
invariance.

Changing a stable component, instance, port, wire, exposure, or transition ID
changes the corresponding identity. Moving a declaration across a composite
boundary changes its occurrence chain and therefore its transition, wire,
mailbox, and Philox draw identities. Such edits are identity-changing
migrations even when the visible scientific structure looks similar.

## Origins and manifest provenance

Sembla distinguishes three input origins:

- **legacy** — an unversioned model JSON document. It has no plan envelope and
  retains the frozen dense positional identity behavior.
- **direct_stable** — a versioned plan exported directly from flat IR. It has a
  stable identity map but no linked-source tuple.
- **linked** — a versioned plan produced from composition source. It embeds its
  source hash, linker descriptor, source map, and complete execution identity
  map.

A plan run records the complete plan tuple:

```json
"plan": {
  "plan_schema": "sembla.executable-plan/v1",
  "identity_scheme": "sembla.identity/stable-v1",
  "origin": "linked",
  "plan_semantic_hash": {
    "algorithm": "sha256",
    "domain": "sembla.plan-core/v1",
    "digest": "…"
  },
  "enabled_features": []
}
```

Linked runs additionally record:

```json
"linked_source": {
  "source_hash": {
    "algorithm": "sha256",
    "domain": "sembla.source-artifact/v1",
    "digest": "…"
  },
  "linker_semantics": "sembla.linker/v1"
}
```

The `linked_source.source_hash` must equal both the plan's embedded provenance
and the bundle source record. `direct_stable` manifests omit `linked_source`;
legacy manifests omit both tuples. These are all-present-or-absent contracts,
not optional hints.
