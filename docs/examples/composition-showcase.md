# Composition showcase

This showcase contains four Lean-authored, executable simulation models built
from ordinary `sembla_component` constants. Together they exercise explicit
parameter bindings, hidden and exposed ports, delayed wiring, fan-out,
repeated components, deep nesting, stable occurrence identities, canonical
source/plan bundles, linked run-manifest provenance, and deterministic replay.

The models are demonstrations rather than calibrated forecasts. They reuse the
small SIR-like population component and synthetic population generator so the
composition mechanics stay visible.

## Model map

| Model | Scientific question | Composition features |
|---|---|---|
| `demo_counterfactual_outbreak` | How does a higher contact rate change the same seeded outbreak? | Repeated reusable regions, explicit parameter rebinding, shared recovery parameter, hidden external policy inputs, independent occurrence/draw identities |
| `demo_coordinated_regions` | What happens when two regions share one policy response? | Cross-region sensing, four delayed wires, one controller output fanned out to two inputs, exposed case-count ports |
| `demo_regional_surveillance` | Can a dashboard observe two locally managed epidemics without feeding back into them? | Nested epidemic-policy components, exposure chains, observation-only dashboard wiring, nested source-map paths |
| `demo_national_network` | How do four regions compose into two regional coordinators and one national view? | Repeated nested composites, four exposure-chain wires, deep occurrence identities, national summaries |

The complete authoring source is
[`frontend/Sembla/Demos/Composition.lean`](../../frontend/Sembla/Demos/Composition.lean).
Compile-time link and identity guards are in
[`CompositionTests.lean`](../../frontend/Sembla/Demos/CompositionTests.lean).

## 1. Build the tools

From the repository root:

```sh
cargo build -p sembla-cli
cd frontend
lake build Sembla.Demos.CompositionTests
cd ..
```

## 2. Inspect or reproduce a bundle

Each checked demonstration bundle has the standard four-file layout under
`fixtures/demos/composition/<model>/`:

```text
bundle-manifest.json
composition-source.json
executable-plan.json
link-report.json
```

For example, reproduce the coordinated-regions bundle:

```sh
mkdir -p /tmp/sembla-composition-demo
(cd frontend && lake exe sembla-export --source demo_coordinated_regions \
  /tmp/sembla-composition-demo/source.json)
(cd frontend && lake exe sembla-link \
  /tmp/sembla-composition-demo/source.json \
  --bundle /tmp/sembla-composition-demo/coordinated.bundle)
target/debug/sembla bundle-verify \
  /tmp/sembla-composition-demo/coordinated.bundle
```

`bundle-verify` checks the canonical manifest, source and plan hashes, report-
sensitive bundle integrity, plan validation/canonicality, and embedded linked
provenance. `bash frontend/scripts/check-parity.sh` reproduces and byte-compares
all four showcase bundles.

## 3. Create one deterministic seeded population

The population file is occurrence-scoped when a plan contains repeated regions:
that gives every leaf a compatible initialized population while preserving its
own stable transition and draw identities.

```sh
target/debug/sembla synth-pop \
  --persons 1000 \
  --employers 50 \
  --initial-infected 600 \
  --seed 123 \
  --out /tmp/sembla-composition-demo/population.bin
```

The intentionally high infected count makes the policy and surveillance
thresholds fire within a short demonstration run.

## 4. Run and replay a model

```sh
MODEL=demo_coordinated_regions
PLAN=fixtures/demos/composition/$MODEL/executable-plan.json
OUT=/tmp/sembla-composition-demo/$MODEL.csv

target/debug/sembla run "$PLAN" \
  --population /tmp/sembla-composition-demo/population.bin \
  --seed 55 \
  --ticks 8 \
  --out "$OUT"

target/debug/sembla verify-run "$OUT.manifest.json" "$PLAN" \
  --population /tmp/sembla-composition-demo/population.bin
```

The run writes:

- `$OUT` — per-tick views and transition firing counts;
- `$OUT.summaries.csv` — the model's declared peak/alert summaries; and
- `$OUT.manifest.json` — execution hashes plus the complete `plan` and
  `linked_source` provenance tuples.

Repeat the commands with any showcase model name from the table. With the
population and run arguments above, the checked summaries are:

| Model | Summary values |
|---|---|
| Counterfactual | `control_peak=631`, `high_contact_peak=745` |
| Coordinated regions | `north_peak=627`, `south_peak=626`, `restricted_ticks=7` |
| Regional surveillance | `north_peak=662`, `south_peak=649`, `alert_ticks=7` |
| National network | `east_north_peak=626`, `east_south_peak=614`, `west_north_peak=617`, `west_south_peak=616`, `national_alert_ticks=7` |

The summary CSV and execution-hash goldens live under
`fixtures/demos/composition/goldens/`. The Rust integration test
`composition_showcase.rs` runs every plan twice, byte-compares CSV, summaries,
and manifests, checks those goldens, then replays each manifest with
`verify-run`.

## 5. Read the identities

Open a plan and inspect `identity.leaves`, `identity.transitions`, and
`identity.mailboxes`.

- Counterfactual leaves include
  `occ:control/population` and `occ:high_contact/population`. Reusing one
  component does not reuse its RNG stream.
- Coordinated-region mailboxes include distinct wires from both case outputs
  into the controller and two fan-out wires from the shared restriction output.
- Regional-surveillance leaves such as `occ:north/population` retain their
  nested component path through exposure wiring.
- National leaves include paths such as
  `occ:east/north/population` and `occ:west/south/population`; moving a region
  across those composite boundaries would intentionally change its identity.

Display-label changes do not affect these identities. Stable instance, port,
wire, or transition renames do. See
[`docs/composition.md`](../composition.md) for the complete grammar and
refactoring rules.

## What each model teaches

### Parallel counterfactuals

`CounterfactualOutbreak` instantiates the same `Region` twice. Both instances
bind `gamma` to `recovery_rate`, while `beta` maps to separate control and
high-contact parameters. Their unused restriction inputs are explicitly
hidden. This is a compact example of explicit sharing without literal
per-instance parameter values.

### Shared policy fan-out

`CoordinatedRegions` sends each region's infection count to one
`RegionalCoordinator`. The controller emits one modifier that is delivered by
two separately identified wires. The output value is shared, but each target
mailbox and endpoint relationship remains explicit in the plan.

### Observation without feedback

`RegionalSurveillance` reuses two complete epidemic-policy composites. Their
infection-count outputs cross composite boundaries through named exposures and
feed a dashboard. The dashboard has no output wire back to either epidemic, so
its alert view is observational only.

### Deep national nesting

`NationalNetwork` repeats the coordinated two-region component twice and feeds
four exposed regional counts into a national dashboard. It demonstrates that
source maps, summaries, mailboxes, transition identities, and run provenance
remain understandable even when occurrence paths are several levels deep.
