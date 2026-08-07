# PRD 0002: ABS acquisition, normalisation, and the committed extracts

## Context

Read `docs/prds-australian-population/README.md` first; its data-vintage table
and the standard-library-only quarantine rule are binding. `DECISIONS.md` §N6
(annual ABS mortality rates; life tables as validation) and §N10 (PRD 0001)
govern.

Nothing downstream is testable without real ABS data: PRD 0003 builds the 2010
initial state from it, PRD 0005 takes fertility and mortality from it, and
PRD 0006 turns it into calibration targets. This PRD is the only place in the
folder that is allowed to touch the network, and even here it is opt-in.

The repository's reproducibility culture applies unchanged. A build that
silently re-downloads, or whose output depends on when it ran, is a failed PRD.

## Goal

`data/abs/` acquires the pinned ABS releases, verifies them by SHA-256,
normalises them into small canonical CSV extracts that are committed to the
repository, and produces a reconciliation report. Every downstream build reads
only the committed extracts and needs no network.

## Specification

### 1. Source manifest — `data/abs/sources.json`

Canonical JSON, one entry per acquired file, each recording: logical `id`, ABS
release title, reference period, release date, retrieval URL, `sha256` of the
exact downloaded bytes, byte length, and the ABS licence statement (CC BY 4.0).
The manifest is the pinned contract; a hash mismatch is a hard failure, never a
warning.

Cover at minimum:

| `id` | Series | Supplies |
|---|---|---|
| `erp_state_age_sex` | National, state and territory population | 30 June ERP by state, single year of age, sex, 2010–2025 |
| `erp_regional_age_sex` | Regional population by age and sex | Sub-state cross-check for the 2010 build |
| `births` | Births, Australia | Annual births by state plus national male/female births for the entrant sex ratio |
| `deaths` | Deaths, Australia | Annual deaths by state, age, sex |
| `mortality_rates` | Deaths, Australia | Annual age-specific death rates per 1,000 by state, sex and five-year band |
| `life_tables_validation` | Life expectancy | Published three-year-period `qx` snapshots for independent validation |
| `nom` | Overseas Migration | Arrivals and departures by state, age, sex |
| `interstate` | National, state and territory population / Migration, Australia | Interstate arrivals and departures by state; origin→destination matrix where published |

### 2. Fetch — `data/abs/fetch.py`

Standard library only (`urllib.request`, `hashlib`, `pathlib`, `json`).
Downloads every manifest entry into `data/abs/cache/<id>.<ext>` and verifies the
SHA-256. Behaviour is frozen:

- runs **only** when invoked with an explicit `--download` flag; without it the
  script verifies the cache and exits non-zero if anything is missing;
- never writes outside `data/abs/cache/`;
- reports each file as `ok`, `missing`, or `hash-mismatch`, and exits non-zero
  on anything but `ok`;
- a `--refresh <id>` path re-downloads one entry and prints the new hash for a
  human to paste into the manifest — it must never rewrite `sources.json`
  itself, because that would let a silent upstream revision pass unnoticed.

`data/abs/cache/` is gitignored.

### 3. Normalisation — `data/abs/normalise.py`

ABS publishes workbook series as `.xlsx` data cubes. Parse them with `zipfile`
plus `xml.etree.ElementTree` — an `.xlsx` is a zip of XML, so this is tractable
without a third-party reader. Read the shared-string table and the sheet XML,
and resolve cell references properly rather than assuming dense rows; ABS cubes
have merged headers, footnote rows and sparse regions, so a positional parser
will silently mis-read. Where the required dimensions are only joint in the ABS
Data API, cache the exact narrow SDMX-CSV response under the same manifest and
hash discipline and parse it with `csv`. A versioned dataflow can still be
revised, so the response bytes are no less strictly pinned than a workbook.

Emit these committed extracts under `data/abs/extracts/`:

| File | Columns |
|---|---|
| `erp_state_age_sex.csv` | `year,state,sex,age,persons` (age `0`–`99`, `100` = 100+) |
| `erp_regional_2010_state_age_sex.csv` | `year,state,sex,age_band,persons` (independent SA2 sum for cross-check) |
| `births_state.csv` | `year,state,births` |
| `births_sex.csv` | `year,sex,births` (national sex ratio; eight-state residual retained) |
| `deaths_state_age_sex.csv` | `year,state,sex,age_band,deaths` |
| `mortality_rates_state_age_sex.csv` | `year,state,sex,age_band,rate_per_1000,status` |
| `mortality_rates_national_age_sex.csv` | `year,sex,age_band,rate_per_1000` (comparison and explicit zero-exposure fallback) |
| `life_tables_state_age_sex.csv` | `period_start,period_end,state,sex,age,qx` (validation only; published periods only) |
| `nom_state_age_sex.csv` | `year,state,sex,age_band,arrivals,departures` |
| `interstate_state_age_sex.csv` | `year,state,sex,age_band,arrivals,departures` (margins, not age-specific O-D) |
| `interstate_flows.csv` | `year,origin,destination,persons` |
| `interstate_margins.csv` | `run_year,state,arrivals,departures` |

Canonical CSV, frozen: LF line endings, a header row, no quoting unless a field
requires it, rows sorted by the full key tuple, integers rendered without
separators, and reals at a fixed decimal precision recorded in the module
docstring. `state` values are the eight lowercase codes from the README's
geography enum.

The `ABS_DEM_QIM` quarterly dataflow covers the whole window. Aggregate its
September-through-June quarters into all 56 directed cells for each run year,
excluding the published zero diagonal. Compare O–D-derived margins with the
separate margin workbooks and report every vintage difference — especially the
material 2020 conflict — rather than scaling either source to force agreement.

### 4. Reconciliation report — `data/abs/reconcile.py`

Writes `data/abs/extracts/reconciliation.md` and exits non-zero on a hard
failure. Check at least:

- state ERP sums to the published national total for every year;
- the ERP cohort identity year over year against published financial-year
  `natural increase + NOM + NIM`, reporting the residual per state per year
  rather than forcing it; calendar registration-year births and deaths are
  reported separately and are never substituted into that identity;
- net interstate migration sums to approximately zero across states each year,
  reporting the actual residual;
- `interstate_flows` margins agree with `interstate_margins` for years where
  both exist;
- no negative counts, no missing (year, state, sex, age) cells, and complete
  coverage of 2010–2025.

The residuals are expected to be nonzero because ERP components are modelled,
confidentialised, constrained and revised. The report states this and quantifies
it; a check that "passes" by zeroing a residual is a failed PRD.

### 5. Tests — `data/abs/tests/`

`unittest`, standard library only, runnable offline against the committed
extracts:

- extract schemas, key completeness and sort order;
- canonical-CSV round trip: re-emitting a parsed extract reproduces the file
  byte for byte;
- the `.xlsx` parser against a small checked-in fixture workbook exercising
  shared strings, sparse rows and a merged header;
- `fetch.py` without `--download` performs no network call — assert by
  monkeypatching `urllib.request.urlopen` to raise;
- the reconciliation checks fire on deliberately corrupted inputs.

New `scripts/check-abs-data.sh` runs the extract tests and re-runs
reconciliation against the committed extracts. Add it to the check matrix in
`CONTRIBUTING.md` and `docs/contributing/ci.md` as a documented local check.

### 6. Documentation — `docs/guides/abs-data.md`

The manifest contract, how to refresh a pinned source and why the hash is pasted
by hand, the extract schemas, the canonical-CSV rules, the vintage table, the
O–D coverage gap if there is one, and ABS attribution. Link from
`docs/models/README.md`.

## Allowed files

- `data/abs/**` (new), `.gitignore` (cache entry only)
- `scripts/check-abs-data.sh` (new)
- `CONTRIBUTING.md`, `docs/contributing/ci.md` (check-matrix entries only)
- `docs/guides/abs-data.md` (new), `docs/models/README.md` (link only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No third-party Python dependency, no `requirements.txt`, no lock file.
- No Rust, Lean, model, fixture or CI workflow change.
- No state-artifact construction — that is PRD 0003.
- No target construction — that is PRD 0006.
- No modelling, interpolation, smoothing or gap-filling of any ABS series.

## Acceptance criteria

1. Full check battery passes, plus `scripts/check-abs-data.sh` and
   `python3 scripts/check-markdown-links.py`; `git diff --check` passes.
2. Every extract in §3 exists, is canonical, and regenerates byte identically
   from the cached raw files. ERP stocks cover 2010–2025; annual event and rate
   extracts cover the fifteen model years 2010–2024; life-table validation rows
   cover exactly the published periods and are never relabelled as annual data.
3. `fetch.py` performs no network access without `--download`, proven by the
   monkeypatched test; a corrupted cache file fails by hash.
4. `sources.json` pins a URL, SHA-256, byte length, reference period and release
   date for every entry, and is never rewritten programmatically.
5. `reconciliation.md` is committed, quantifies the ERP component residuals per
   state per year, and states why they are nonzero.
6. All 56 origin→destination cells exist for every run year; their margins are
   compared with the separate margin workbooks, and the 2020 vintage conflict
   is reported in both reconciliation and documentation with no altered data.
7. The pipeline imports no Sembla crate, library or API, and no module outside
   the Python standard library.
