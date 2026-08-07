# Australian initial state: 30 June 2010

This report is generated from the committed ABS extracts by
`python3 data/abs/build_state.py --write-report`. Counts are people at
`full` scale and slots at the named reduced scale.

## Pool arithmetic

The two entrant streams are single-use. Every initially present or exited
row is `retired_slot`, so internal movement and retired rows cannot consume
the pre-classified entry pools.

| component | observed requirement | 10% headroom | full slots |
|---|---:|---:|---:|
| present ERP, 2010 | 22,028,695 | 0 | 22,028,695 |
| registered births, 2010–2024 | 4,552,773 | 455,278 | 5,008,051 |
| overseas arrivals, 2010–2024 | 7,462,880 | 746,288 | 8,209,168 |
| **total** | **34,044,348** | **1,201,566** | **35,245,914** |

Headroom is rounded up independently before scale reduction. It is a
capacity choice, not a claim that saturation is impossible: PRD 0008 fails
a calibration whose birth or overseas vacancy margin reaches zero or
approaches it without an explicitly justified reserve.

## Scale outputs

| scale | divisor | present | birth slots | overseas slots | total rows |
|---|---:|---:|---:|---:|---:|
| `full` | 1 | 22,028,695 | 5,008,051 | 8,209,168 | 35,245,914 |
| `tenth` | 10 | 2,202,870 | 500,805 | 820,917 | 3,524,592 |
| `hundredth` | 100 | 220,287 | 50,081 | 82,092 | 352,460 |

The sum of separately rounded stream capacities can differ by one from
rounding the combined full pool. Stream capacities are authoritative
because each stream has its own eligibility guard and saturation metric.

## ERP fidelity and rounding

State and single-year age margins are first apportioned to the national
target. A deterministic minimum-cost bipartite allocation then chooses
cell floor-to-ceiling increments, maximising remainders with sorted cell
keys as the exact tie-break.

| scale | national total exact | all 8 state margins exact | all 101 age margins exact | cell error range (agents) | cell MAE | zero cells | one-agent cells |
|---|---|---|---|---:|---:|---:|---:|
| `full` | yes | yes | yes | +0.00 to +0.00 | 0.000000 | 2 | 4 |
| `tenth` | yes | yes | yes | -0.60 to +0.70 | 0.251547 | 13 | 11 |
| `hundredth` | yes | yes | yes | -0.63 to +0.69 | 0.254660 | 52 | 42 |

At `full`, every one of the 1,616 `(state, sex, age)` cells equals the
published ERP count exactly. At `hundredth`, NT has 29 zero and 14 one-agent cells; ACT has 13 zero and 7 one-agent cells. These are the accepted small-cell limitation and are never smoothed.

## Entrant composition

The eight-state birth requirement is 4,552,773. The
national sex series contains 2,339,145 male and
2,214,101 female births; its 473-person
excess is the retained Australia/eight-state residual. Birth slots preserve
the apportioned state and sex margins exactly.

Detailed NOM arrivals sum to 7,464,590, which is
+1,710 relative to the separately published gross
margin used for pool size. Overseas slots preserve apportioned state × sex
and age-band margins exactly. `entry_age_months` is the published band's
lower bound times 12, including 65+ as 780; no within-band observations are
manufactured.

Entrant composition is fixed when the artifact is built. Calibration fits
birth and overseas *rates*, not their state, sex or age mix.

## Row encoding

Present rows are sorted by `(state, sex, age, within-cell index)`; their
single-year ages are spread by `age * 12 + floor(index * 12 / cell_count)`.
Birth groups follow in `(state, sex)` order, then overseas groups in
`(state, sex, published age band)` order. Row ordinal is the Philox entity
coordinate and the row's `slot_resource` reference. Every initial event is
`none_`, every `prev_area` is `none_`, and every generation is 0.

## Artifact evidence

`state hash` is SHA-256 over `sembla.state-artifact/v1 || 0x00 || bytes`,
the exact record printed by Rust `sembla state-hash`. `file SHA-256` is the
ordinary digest used for file pinning.

| scale | path | bytes | file SHA-256 | state hash |
|---|---|---:|---|---|
| `full` | `data/abs/generated/australian_population_2010_full.state` | 1,691,804,647 | `55429a20f03d2a0f63d1490ca13e0191579451043c99b573bed037fb398461ae` | `ef3c93722199cf96b4d7839d79561de1f4c0e944924c4a9d490e64ba6a92c083` |
| `tenth` | `data/abs/generated/australian_population_2010_tenth.state` | 169,181,189 | `b6f7e827df3da3d247f4194194c0ed3770e16677bc71b739cd617c19cbd96ee2` | `926fc80a0330c764cac8c4a69c09dee6b4d088653dc9bf830c9e068a2b87c02a` |
| `hundredth` | `fixtures/state/australian_population_2010_hundredth.state` | 16,918,851 | `1d3f85db8fd93c66118df15622c70eac4fd6dfc1adcc72c9142b5949146eff5f` | `c7db0d7324aecd9a50a3d297e604f71da8677058c20ae9b42f8fd7524a136df4` |

`hundredth` and its paired model are committed. `full` and `tenth` are
generated on demand under ignored `data/abs/generated/`. Each validation-safe
companion changes the two table `size_hint` values and omits only the
feature-gated `grouped_views`; parameters, dynamics and scalar views remain
identical to the canonical execution model.
The full regeneration, Rust hashes and negative row-count checks are recorded
in `docs/evidence/australian-population/initial-state-2010/README.md`.

## Cross-language conformance

- The generic Python writer reproduces Rust's committed `refs_small.state`
  bytes and frozen state hash exactly.
- `AustralianPopulation.lean` generates 56 move and 336 mortality transitions
  and its post-splice model passes Lean `checkModel`.
- Public `sembla validate` accepts both the validation-safe `.state.model.json`
  companion and the canonical feature-bearing executable plan. A structural
  test proves that only grouped views and scale row counts differ.
- `cargo test -p sembla-cli --test australian_population` checks schema,
  rows, stream counts, ordinals, companion equivalence and the Rust state
  hash, then proves identity and retirement for every slot across all 24 golden
  boundaries, exact interstate conservation, state/national stock-flow identities,
  closed accounting and deterministic replay.
- A zero-tick export is not honest because summaries cannot reduce an empty
  run. Conformance therefore uses Rust `state-hash` plus exact loader values,
  rather than pretending a one-tick changed state is a round trip.
