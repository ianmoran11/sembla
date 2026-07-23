# Composition guide

Sembla's first composition release has one executable path:

```text
Lean surface syntax → canonical composition source → Lean linker
  → canonical executable plan → Rust validation → deterministic run
```

The normative rules are in [`DECISIONS.md` §J](../DECISIONS.md#j-composition-and-the-option-d-architecture-accepted-2026-07-21).
The architecture and rollout boundaries are described in
[`docs/design/option-d-architecture.md`](design/option-d-architecture.md).
For four runnable worked models, see the
[`composition showcase`](examples/composition-showcase.md).

## Author components and a root composition

`sembla_component` defines an ordinary Lean constant. A primitive component
uses the same `system`, `input`, transition, `output`, and `view` declarations
as `sembla_model`; `requires` names model-level parameters used by its body.
Components do not declare `dt`.

A composite component instantiates component constants and explicitly wires,
exposes, or hides their ports:

```lean
sembla_component EpidemicPolicy where
  instance population := Population (
    beta := beta,
    gamma := gamma)
  instance policy := Policy
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
  param beta : ℝ := 0.3
  param gamma : ℝ := 0.1
  root EpidemicPolicy
  summary infected_peak := max population.I
```

See
[`frontend/Sembla/Composition/SurfaceModels.lean`](../frontend/Sembla/Composition/SurfaceModels.lean)
for complete primitive, composite, nested, and root examples.

## Inspecting compositions

Place the cursor on a `sembla_component` or `sembla_composition` declaration
name to open its composition structure panel in the Lean infoview. The panel
shows the authored level only: instance boxes identify primitive versus
composite definitions and list their boundary input/output ports. Nested
children remain collapsed; move to the child component declaration to inspect
that level. Primitive components show their interface summary without running
a simulation.

Connections deliberately teach the delay discipline. A solid wire row is a
mailbox connection and carries an explicit `1-tick delay` marker. A dashed
exposure row is a `zero-delay alias`: it renames a child boundary at the parent
boundary without adding a mailbox tick. Hidden child ports appear struck
through in a separate row. The panel is built only from already-elaborated
composition values, so inspection adds no runtime behavior or execution cost.

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

Plan envelopes run on the deterministic CPU oracle and may select CUDA with
`--backend cuda` when a qualified device is available. Composition does not
add a second runtime semantics: the linker emits the same flat model shape
consumed by existing validation and execution.

## Sweep a plan for calibration

A linked plan uses the existing sweep, prior, noise, and summary machinery.
For example, generate a hermetic population and export independent-noise
`(θ, x)` pairs from the `two_regions` plan's `beta`/`gamma` priors and
`peak_i` summary:

```sh
cargo run -p sembla-cli -- synth-pop \
  --persons 1000 --employers 50 --initial-infected 600 --seed 123 \
  --out build/population.bin
cargo run -p sembla-cli -- sweep fixtures/plans/linked/two_regions.plan.json \
  --population build/population.bin --seed 91 --draws 100 --ticks 40 \
  --noise independent --out build/two-regions-sweep \
  --export-pairs build/two-regions-pairs.csv
```

The sweep directory has the same draw CSVs, aggregate summary, θ manifest,
and canonical run manifest as a legacy sweep. The pairs CSV and adjacent
`.meta.json` sidecar retain the calibration export format. A plan sweep's run
manifest records the complete plan identity tuple and linked-source tuple,
not a legacy `ir_hash`; direct-stable plans omit only `linked_source`.

## Compare plans with common random numbers

`compare` accepts either two plan envelopes or one plan with two parameter
files. Both arms use the same seed. Content-addressed transition identities
make that pairing principled: a shared component keeps the same `occ:…#…`
identity, rule word, and Philox draws even when the surrounding composed model
changes (DECISIONS.md §J4 and §J14).

For example, the population leaf is shared by the standalone plan and the
unwired population-plus-policy product. Every population view and firing
trajectory in this contrast is therefore exactly equal, not merely
statistically similar:

```sh
cargo run -p sembla-cli -- compare \
  fixtures/plans/linked/solo_population.plan.json \
  fixtures/plans/linked/independent_epidemic_policy.plan.json \
  --population build/population.bin --seed 55 --ticks 8 \
  --out build/population-noninterference.csv
```

The wired policy regression fixture exposes the otherwise literal restriction
threshold as a test-only parameter. At this seed, thresholds 500 and 1000 have
identical population columns at ticks 0 and 1; the first difference is pinned
at tick 2, after the population-to-policy and policy-to-population one-tick
wires have both carried the counterfactual:

```sh
printf '%s\n' '{"restriction_threshold":500}' > build/policy-a.json
printf '%s\n' '{"restriction_threshold":1000}' > build/policy-b.json
cargo run -p sembla-cli -- compare \
  crates/sembla-cli/tests/fixtures/epidemic_policy_threshold.plan.json \
  --population build/population.bin --seed 55 --ticks 8 \
  --params-a build/policy-a.json --params-b build/policy-b.json \
  --out build/policy-counterfactual.csv
```

The committed counterfactual demo is also directly runnable in parameter
contrast form without changing any demo artifact:

```sh
printf '%s\n' '{"control_beta":0.45}' > build/control.json
printf '%s\n' '{"control_beta":0.9}' > build/high-contact.json
cargo run -p sembla-cli -- compare \
  fixtures/demos/composition/demo_counterfactual_outbreak/executable-plan.json \
  --population build/population.bin --seed 55 --ticks 8 \
  --params-a build/control.json --params-b build/high-contact.json \
  --out build/demo-counterfactual.csv
```

Legacy models may still compare with legacy models, but a legacy/plan pair is
rejected before execution because positional and stable identities cannot
form a meaningful CRN contrast.

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
