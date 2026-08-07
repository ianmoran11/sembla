# ABS data pipeline

`data/abs/` acquires the pinned ABS releases, normalises them into small
canonical extracts that are committed to the repository, and reconciles them.
Every downstream build reads only the committed extracts and needs no network.

The pipeline is quarantined and uses the **Python standard library only**: it
never imports a Sembla crate, library, model parser or runtime API, and takes no
third-party dependency at all (`DECISIONS.md` §N10). Population construction
stays outside the runtime boundary, as `DESIGN.md` §10.5 requires.

## Layout

| Path | Role |
|---|---|
| `sources.json` | Pinned manifest: URL, SHA-256, byte length, release and reference period for every acquired file |
| `cache/` | Raw downloads, gitignored, verified by hash |
| `extracts/` | Canonical committed CSV plus reconciliation and initial-state reports |
| `xlsx.py` | Minimal read-only XLSX reader with streaming rows for multi-million-cell cubes |
| `absts.py` | ABS time-series workbook reader |
| `sdmx_csv.py` | Pinned ABS Data API CSV reader and dimension-code maps |
| `canonical.py` | Canonical CSV and JSON writers |
| `fetch.py` | Acquisition and cache verification |
| `normalise.py` | Extract construction |
| `reconcile.py` | Cross-checks and the source reconciliation report |
| `scaling.py` | Deterministic constrained Hare–Niemeyer apportionment |
| `state_artifact.py` | Generic streaming `sembla.state/v1` writer driven by exported model JSON |
| `build_state.py` | Three-scale 2010 slot allocation, artifacts and build report |
| `rates.py` / `params/` | Annual fixed-rate derivation, prior registry and fidelity evidence |
| `targets.py` / `targets/` | Versioned annual target ledgers, hashes and reduction evidence |
| `score.py` | Strict run-to-target residual reports and ordered summary vectors |

## Commands

```bash
python3 data/abs/fetch.py                  # verify the cache; no network
python3 data/abs/fetch.py --download       # acquire missing or mismatched files
python3 data/abs/fetch.py --refresh <id>   # re-download one file, print its hash
python3 data/abs/normalise.py              # rebuild the extracts
python3 data/abs/reconcile.py              # rebuild reconciliation.md
python3 data/abs/build_state.py --scale hundredth --plan-only
python3 data/abs/build_state.py --write-report
python3 data/abs/build_state.py --scale hundredth \
  --model fixtures/australian-population/australian_population.hundredth.json
python3 data/abs/rates.py                  # rebuild annual params and rates.md
python3 data/abs/gravity_fit.py            # refit spatial parameters to the O-D table
python3 data/abs/targets.py                # rebuild target ledgers and index
./scripts/check-abs-data.sh                # data, report and 1:100 artifact checks
```

`gravity_fit.py` is the offline stage of migration calibration: a standard-library
Poisson log-linear fit of the fifteen identifiable spatial parameters
(`interstate_base`, seven pushes, seven pulls; `push_nsw` and `pull_nsw` are
fixed at one) to the 56 published origin-destination cells of each run year,
leaving 41 residual degrees of freedom. It writes full 377-parameter files to
`data/abs/params/gravity/` with only the fifteen free slots changed, plus a
`fit-report.json` carrying every cell's observed, expected, signed and Pearson
residuals, deviance contributions, and the comparison against the separately
published margins — including the 2020 vintage conflict, which is reported and
never reconciled. The age-profile parameters `peak_months` and `k` are held at
their prior centres there; the all-age table cannot identify them. The full
two-stage design is in the
[calibration guide](australian-population-calibration.md).


`fetch.py` performs no network access without `--download`; a test enforces this
by making `urllib.request.urlopen` raise during verification. The target schema,
held-out split, scale reconciliation, scorer and measured full/reduced decision
are documented in the [targets and scoring guide](targets.md).

## Pinning and refreshing

`sources.json` records the SHA-256 of the exact bytes of every source. A
mismatch is a hard failure, never a warning.

The manifest is **never rewritten programmatically**. `--refresh` re-downloads a
single entry and prints the observed hash for a human to paste in after
reviewing the upstream change. This is deliberate: ABS revises and rebases its
estimates, and an auto-updating manifest would absorb a revision silently and
change every downstream artifact without review.

Workbook URLs are pinned to a specific issue (for example `.../dec-2025/`), not
to `latest-release`. Required joint dimensions that the workbooks do not carry
— state births, state-age-sex deaths, state-age-sex overseas migration, and
state departure×arrival — come from narrow, versioned ABS Data API queries. The **exact raw
CSV response bytes** are cached and pinned by the same URL, byte-count and hash
contract as workbooks. The API serves a current vintage even when its dataflow
version remains `1.0.0`, so a later revision fails verification and requires the
same explicit human review as a revised workbook.

## Pinned vintages

| Series | Pinned reference period | Release date | Manifest IDs |
|---|---|---:|---|
| National/state ERP and components | December 2025; annual ERP through 30 June 2025 | 2026-06-18 | `components_*`, `erp_*`, `interstate_*` |
| Regional ERP by SA2, age and sex | 2001–2024 back-series | 2025-08-28 | `erp_regional_age_sex` |
| Registered births | 2010–2024 | 2025-10-15 | `births_state`, `births_sex` |
| Registered deaths and annual mortality rates | 2010–2024 | 2025-09-26 | `deaths_state_age_sex`, `mortality_rates_state_age_sex` |
| Overseas migration | 2010-11–2024-25 extracts from the 2024-25 release | 2025-12-19 | `nom_state_age_sex`, `overseas_*` |
| Period life tables | 2018–2020 through 2022–2024 | 2021-11-04–2025-11-11 | `life_tables_*` |

The release period and observation period are deliberately separate. For
example, the December 2025 ERP release revises historical observations, while
the pinned births and deaths API responses expose their 2024 observation
endpoint over a 2010–2024 query window.

## Extracts

All are canonical: LF endings, header row, rows sorted by the full key tuple,
integers without separators, and real values rendered at a fixed nine decimal
places before trailing-zero trimming.

| File | Columns |
|---|---|
| `erp_state_age_sex.csv` | `year,state,sex,age,persons` — 30 June ERP, ages 0–100 where 100 is the open terminal group |
| `erp_national_age_sex.csv` | `year,region,sex,age,persons` — the published Australia series, for reconciliation |
| `erp_regional_2010_state_age_sex.csv` | `year,state,sex,age_band,persons` — SA2 back-series summed independently for the 2010 cross-check |
| `components_state.csv` | `run_year,state,natural_increase,net_overseas_migration,net_interstate_migration` |
| `births_state.csv` | `year,state,births` — final calendar registration-year counts |
| `births_sex.csv` | `year,sex,births` — national male/female registrations used only for the entrant sex ratio |
| `deaths_state_age_sex.csv` | `year,state,sex,age_band,deaths` — final calendar registration-year counts; includes `not_stated` age |
| `mortality_rates_state_age_sex.csv` | `year,state,sex,age_band,rate_per_1000,status` — annual model input; two zero-exposure cells remain explicitly blank |
| `mortality_rates_national_age_sex.csv` | `year,sex,age_band,rate_per_1000` — national comparison and the two-cell fallback source |
| `life_tables_state_age_sex.csv` | `period_start,period_end,state,sex,age,qx` — five overlapping period snapshots, validation only |
| `nom_state_age_sex.csv` | `year,state,sex,age_band,arrivals,departures` — financial year labelled by its starting/run year |
| `interstate_flows.csv` | `year,origin,destination,persons` — all 56 directed cells for every run year |
| `interstate_state_age_sex.csv` | `year,state,sex,age_band,arrivals,departures` — age/sex margins, not age-specific O-D |
| `interstate_margins.csv` | `run_year,state,arrivals,departures` |
| `overseas_margins.csv` | `run_year,state,arrivals,departures` |
| `components_national.csv` | `run_year,births,deaths,overseas_arrivals,overseas_departures` |

`initial-state-2010.md` records the 10% per-stream headroom, all three pool
sizes, constrained-rounding residuals and both ordinary and domain-separated
artifact hashes. The 1:100 artifact and paired model are committed under
`fixtures/state/`; 1:10 and full artifacts regenerate under ignored
`data/abs/generated/`.

A **run year** is 1 July to 30 June, matching the model's annual runs
(`DECISIONS.md` §N7). Run year *Y* carries stocks from 30 June *Y* to 30 June
*Y+1*, so its flows are the September, December, March and June quarters that
follow 30 June *Y*. A run year is emitted only when all four quarters are
present, so a partial year is dropped rather than silently under-counted.

Stocks cover 2010–2025; flows cover the fifteen run years 2010–2024.
`nom_state_age_sex.csv` follows that financial-year convention even though the
SDMX source labels each observation by its **ending** June (`TIME_PERIOD=2011`
means 2010-11 and is emitted as year 2010). Birth and death extracts instead
retain ABS's calendar registration year. That mismatch is visible and is not
silently shifted or scaled.

## What reconciliation checks, and what it only reports

`extracts/reconciliation.md` is regenerated by `reconcile.py`, which exits
non-zero on a hard failure. Three results are worth understanding, because each
looks like an error and is not.

**The regional 2010 cross-check closes exactly.** The independently structured
SA2 back-series contains 2,454 rows for 2010. After excluding state code 9
(Other Territories), summing SA2s reproduces every state-sex-age-band cell from
the state ERP workbooks exactly. Regional data validates the state extraction;
it does not alter it.

**The eight states do not sum to published Australia.** National ERP covers
Australia, which is the eight states and territories *plus Other Territories*
(Christmas Island, the Cocos (Keeling) Islands and Jervis Bay Territory, joined
by Norfolk Island from 2016). The shortfall runs from about 3,000 in 2010 to
about 5,000 in 2025, and the 2015→2016 step of +1,757 is Norfolk Island entering
scope — which independently corroborates the explanation. The model's geography
is the eight states, so **any comparison against a national figure must use the
eight-state sum**, not published Australian ERP.

**The stock-flow residual is nonzero before 2021 and exactly zero after.** The
identity checked is

```text
ERP(y+1) - ERP(y) = natural increase + net overseas migration
                    + net interstate migration + intercensal discrepancy
```

Estimates after the most recent Census rebase are carried forward from the
components themselves, so the identity closes exactly. Before it, the
discrepancy between successive Census bases has been distributed across the
intercensal years, so a residual remains. It is reported per state and run year
and never forced away (`DECISIONS.md` §N13).

**Net interstate migration does not sum to exactly zero across the eight
states.** Every interstate move is one region's arrival and another's departure,
so it sums to zero across *all* regions — but Other Territories also exchange
population with the states and are not published as a series in this table. The
residual is at most 22 persons in a year and usually zero.

**The quarterly state O-D matrix covers the full window.** The versioned
`ABS_DEM_QIM` dataflow supplies all 56 directed state pairs from September 2010
through June 2025, aggregated here into 840 annual cells. O-D-derived margins
match the separate ERP margin workbooks exactly in 2016–2019 and 2021–2024 and
within 500 persons before 2016. They diverge materially in run year 2020 (worst
27,626) because the two products carry different COVID-era revisions. Both are
preserved; neither is forced to the other. Separate financial-year age/sex
margins also cover 2010–2024 and sum to the headline margins within 65 persons
after cell rounding; they do not make age jointly observed with O-D.

**Overseas margins agree with published net overseas migration to within
rounding.** Arrivals and departures come from the Overseas Migration release
while net overseas migration comes from the ERP components table. The two are
published independently, yet implied net matches published net to within 8
persons in most years — a strong independent check on both extracts. Run year
2020 differs by 84 (COVID-era revision) and 2024 by about 3,300, because the
most recent financial year is preliminary in both releases and revised on
different schedules. Treat the final run years as provisional.

**Overseas age-sex detail reproduces the independently extracted margins.** The
SDMX detail and workbook totals are separate physical sources. Each detailed
cell is rounded to the nearest 10; despite accumulating 28 cells per state-year,
the detailed arrivals and departures differ from their separately rounded
margins by at most 50 persons. This check also caught and now freezes the SDMX
convention that a financial year is labelled by its ending year.

**Births and deaths are complete as registration-year evidence.** Direct state
births and state-age-sex deaths cover 2010-2024. Eight-state totals sit just
below published Australia totals because the latter include Other Territories:
19 births and 22 deaths in 2010, and 24 births and 46 deaths in 2024.

**Annual mortality rates are complete except for two explicit zero-exposure
cells.** ABS publishes all 5,040 state-sex-band-year positions except NT female
100+ in 2010 and 2011. The extract keeps those two rates blank with status
`not_published_zero_exposure`; PRD 0005 uses the same-year Australian female
100+ rate rather than interpreting absence as zero mortality.

**Life tables remain period-labelled validation.** Five XLSX releases from
2018–2020 through 2022–2024 contribute 8,080 state-sex-single-age `qx` values.
The extract retains both period boundaries, and no three-year snapshot is
relabeled as annual evidence. Under §N6 it validates the independently
published annual mortality rates but does not drive model parameters.

## Remaining coverage limitation

This is recorded in `sources.json` under `coverage_gaps` and is not silently
worked around.

**Birth/death temporal basis.** The direct extracts are final calendar
registration-year counts. ERP natural increase is a financial-year component
based on occurrence and demographic adjustment. Applying the registration
series to model run years therefore requires an explicit convention in PRD
0005; this acquisition layer preserves both quantities and does not force an
identity.

## Attribution

ABS data are licensed under
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/).
Source: Australian Bureau of Statistics.
