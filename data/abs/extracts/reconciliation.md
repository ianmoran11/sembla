# ABS extract reconciliation

Years covered: 2010-2025 (16 annual points).
States: nsw, vic, qld, sa, wa, tas, nt, act.
Ages: 0-100, where 100 is the open terminal group.

## Structural completeness

- PASS: state and national ERP both cover the fixed years 2010-2025
- PASS: state cells complete: 25856 of 25856 expected
- PASS: no negative state counts
- PASS: no negative national counts
- PASS: no missing (year, state, sex, age) cells [0 missing]

## Regional 2010 cross-check

- PASS: 2010 SA2 sums reproduce every state-sex-age-band ERP cell exactly

The regional workbook is an independently structured SA2 back-series on 2021 ASGS boundaries. Exact agreement after summing 2,454 SA2 rows verifies both the 2010 state extraction and the exclusion of Other Territories; no regional values are used to alter the state ERP.

## State totals against the published national series

The national series covers Australia, which is the eight states and territories **plus Other Territories** (Christmas Island, the Cocos (Keeling) Islands and Jervis Bay Territory, joined by Norfolk Island from 2016). The model's geography is the eight states only (DECISIONS.md N1), so the national total is expected to exceed the sum of states by the Other Territories population. The invariant tested is therefore that the residual is non-negative and small, not that it is zero.

- PASS: national total is never below the sum of the eight states
- PASS: Other Territories residual stays below 0.05% of national [worst 0.0190%]
- PASS: no cell where the states exceed the national figure [0 such cells]

Implied Other Territories population by year:

| year | national | eight states | Other Territories |
|---|---:|---:|---:|
| 2010 | 22,031,750 | 22,028,695 | 3,055 |
| 2011 | 22,340,024 | 22,336,907 | 3,117 |
| 2012 | 22,733,465 | 22,730,432 | 3,033 |
| 2013 | 23,128,129 | 23,125,167 | 2,962 |
| 2014 | 23,475,686 | 23,472,790 | 2,896 |
| 2015 | 23,815,995 | 23,813,144 | 2,851 |
| 2016 | 24,190,907 | 24,186,299 | 4,608 |
| 2017 | 24,592,588 | 24,587,917 | 4,671 |
| 2018 | 24,963,258 | 24,958,533 | 4,725 |
| 2019 | 25,334,826 | 25,330,050 | 4,776 |
| 2020 | 25,649,248 | 25,644,445 | 4,803 |
| 2021 | 25,685,412 | 25,680,562 | 4,850 |
| 2022 | 26,018,721 | 26,013,800 | 4,921 |
| 2023 | 26,659,922 | 26,654,952 | 4,970 |
| 2024 | 27,194,286 | 27,189,291 | 4,995 |
| 2025 | 27,611,026 | 27,606,008 | 5,018 |

The +1,757 step between 2015 and 2016 is Norfolk Island entering the estimated resident population, which independently corroborates that this residual is Other Territories rather than a parsing error.

**Scope consequence.** The model does not represent Other Territories, so its national total is the sum of the eight states and is below published Australian ERP by the amounts above. Any comparison against a national figure must use the eight-state sum.

## Annual population by state

| year | nsw | vic | qld | sa | wa | tas | nt | act | total |
|---|---|---|---|---|---|---|---|---|---|
| 2010 | 7,144,292 | 5,461,101 | 4,404,744 | 1,627,322 | 2,290,845 | 508,847 | 229,778 | 361,766 | 22,028,695 |
| 2011 | 7,218,529 | 5,537,817 | 4,476,778 | 1,639,614 | 2,353,409 | 511,483 | 231,292 | 367,985 | 22,336,907 |
| 2012 | 7,304,244 | 5,651,091 | 4,568,687 | 1,656,725 | 2,425,507 | 511,724 | 235,915 | 376,539 | 22,730,432 |
| 2013 | 7,404,032 | 5,772,669 | 4,652,824 | 1,671,488 | 2,486,944 | 512,231 | 241,722 | 383,257 | 23,125,167 |
| 2014 | 7,508,353 | 5,894,917 | 4,719,653 | 1,686,945 | 2,517,608 | 513,621 | 242,894 | 388,799 | 23,472,790 |
| 2015 | 7,616,168 | 6,022,322 | 4,777,692 | 1,700,668 | 2,540,672 | 515,117 | 244,692 | 395,813 | 23,813,144 |
| 2016 | 7,732,858 | 6,173,172 | 4,845,152 | 1,712,843 | 2,555,978 | 517,514 | 245,678 | 403,104 | 24,186,299 |
| 2017 | 7,855,316 | 6,302,608 | 4,926,380 | 1,728,673 | 2,585,720 | 526,762 | 247,412 | 415,046 | 24,587,917 |
| 2018 | 7,954,476 | 6,423,038 | 5,006,623 | 1,746,137 | 2,617,792 | 537,291 | 247,095 | 426,081 | 24,958,533 |
| 2019 | 8,046,748 | 6,537,305 | 5,088,847 | 1,767,395 | 2,659,625 | 547,841 | 246,559 | 435,730 | 25,330,050 |
| 2020 | 8,110,610 | 6,615,046 | 5,165,613 | 1,790,355 | 2,712,912 | 557,578 | 247,428 | 444,903 | 25,644,445 |
| 2021 | 8,097,062 | 6,547,822 | 5,215,814 | 1,802,601 | 2,749,365 | 567,239 | 248,151 | 452,508 | 25,680,562 |
| 2022 | 8,182,098 | 6,615,232 | 5,310,905 | 1,824,969 | 2,793,934 | 572,300 | 253,100 | 461,262 | 26,013,800 |
| 2023 | 8,356,387 | 6,797,169 | 5,450,235 | 1,857,646 | 2,890,936 | 573,731 | 257,508 | 471,340 | 26,654,952 |
| 2024 | 8,492,050 | 6,950,961 | 5,571,890 | 1,882,164 | 2,978,147 | 574,765 | 260,884 | 478,430 | 27,189,291 |
| 2025 | 8,590,113 | 7,069,856 | 5,669,915 | 1,901,615 | 3,046,209 | 577,770 | 265,895 | 484,635 | 27,606,008 |

## Year-on-year national change

| year | population | change | growth |
|---|---:|---:|---:|
| 2010 | 22,028,695 | - | - |
| 2011 | 22,336,907 | +308,212 | +1.40% |
| 2012 | 22,730,432 | +393,525 | +1.76% |
| 2013 | 23,125,167 | +394,735 | +1.74% |
| 2014 | 23,472,790 | +347,623 | +1.50% |
| 2015 | 23,813,144 | +340,354 | +1.45% |
| 2016 | 24,186,299 | +373,155 | +1.57% |
| 2017 | 24,587,917 | +401,618 | +1.66% |
| 2018 | 24,958,533 | +370,616 | +1.51% |
| 2019 | 25,330,050 | +371,517 | +1.49% |
| 2020 | 25,644,445 | +314,395 | +1.24% |
| 2021 | 25,680,562 | +36,117 | +0.14% |
| 2022 | 26,013,800 | +333,238 | +1.30% |
| 2023 | 26,654,952 | +641,152 | +2.46% |
| 2024 | 27,189,291 | +534,339 | +2.00% |
| 2025 | 27,606,008 | +416,717 | +1.53% |

## Stock-flow identity

- PASS: components and interstate margins both cover fixed run years 2010-2024

For each state and run year, the change in estimated resident population from 30 June to 30 June is compared against the published components of that change:

```text
ERP(y+1) - ERP(y) = natural increase + net overseas migration
                    + net interstate migration + intercensal discrepancy
```

The residual is the intercensal discrepancy and rebasing. It is reported, never forced to zero (DECISIONS.md N13). Expect it to vanish for run years after the most recent Census rebase, where estimates are carried forward from the components themselves, and to be nonzero before it, where the discrepancy between successive Census bases has been distributed across the intercensal years.

- PASS: stock-flow identity evaluated for all 120 state-year cells
- PASS: largest state-year residual stays well below 1% of national population [worst 20,871]

Residual by state and run year:

| run year | nsw | vic | qld | sa | wa | tas | nt | act |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 2010 | -13,940 | -6,621 | -5,194 | -1,332 | -22 | -512 | +18 | -255 |
| 2011 | -1,817 | +15,833 | -1,844 | +479 | -7,261 | -1,424 | -1,006 | -174 |
| 2012 | -1,975 | +16,170 | -2,095 | +483 | -7,428 | -1,469 | -1,008 | -197 |
| 2013 | -1,263 | +16,384 | -2,303 | +515 | -7,670 | -1,516 | -1,023 | -224 |
| 2014 | -641 | +17,489 | -2,373 | +669 | -7,825 | -1,483 | -1,072 | -198 |
| 2015 | +2,067 | +20,871 | -1,864 | +1,565 | -7,667 | -1,328 | -870 | -156 |
| 2016 | -11,817 | -14,553 | -948 | +4,272 | +9,494 | +3,933 | -1,039 | +1,336 |
| 2017 | -12,245 | -14,768 | -1,417 | +4,381 | +9,557 | +3,994 | -1,012 | +1,311 |
| 2018 | -13,172 | -14,876 | -2,051 | +4,459 | +9,711 | +4,070 | -980 | +1,324 |
| 2019 | -14,220 | -14,342 | -3,006 | +4,457 | +9,656 | +4,101 | -1,021 | +1,256 |
| 2020 | -16,369 | -13,830 | -6,710 | +4,874 | +9,464 | +4,205 | -1,187 | +1,152 |
| 2021 | +0 | +0 | +0 | +0 | +0 | +0 | +0 | +0 |
| 2022 | +0 | +0 | +0 | +0 | +0 | +0 | +0 | +0 |
| 2023 | +0 | +0 | +0 | +0 | +0 | +0 | +0 | +0 |
| 2024 | +0 | +0 | +0 | +0 | +0 | +0 | +0 | +0 |

Residual by run year, summed over states (absolute):

| run year | signed residual | absolute residual |
|---|---:|---:|
| 2010 | -27,858 | 27,894 |
| 2011 | +2,786 | 29,838 |
| 2012 | +2,481 | 30,825 |
| 2013 | +2,900 | 30,898 |
| 2014 | +4,566 | 31,750 |
| 2015 | +12,618 | 36,388 |
| 2016 | -9,322 | 47,392 |
| 2017 | -10,199 | 48,685 |
| 2018 | -11,515 | 50,643 |
| 2019 | -13,119 | 52,059 |
| 2020 | -18,401 | 57,791 |
| 2021 | +0 | 0 |
| 2022 | +0 | 0 |
| 2023 | +0 | 0 |
| 2024 | +0 | 0 |

## Interstate migration consistency

- PASS: origin-destination flows cover all 56 directed cells in every run year
- PASS: origin-destination flows are non-negative and exclude diagonal cells
- PASS: O-D-derived margins exactly match the separate margin workbooks in 2016-2019 and 2021-2024
- PASS: pre-2016 O-D margins stay within 500 persons of the revised margin workbooks

| run year | worst O-D versus margin difference |
|---|---:|
| 2010 | 403 |
| 2011 | 49 |
| 2012 | 37 |
| 2013 | 45 |
| 2014 | 36 |
| 2015 | 100 |
| 2016 | 0 |
| 2017 | 0 |
| 2018 | 0 |
| 2019 | 0 |
| 2020 | 27,626 |
| 2021 | 0 |
| 2022 | 0 |
| 2023 | 0 |
| 2024 | 0 |

The 2020 difference (worst 27,626) is real published-vintage evidence, not a parsing residual: the quarterly O-D dataflow and the separately revised ERP margin workbooks diverge during the COVID interstate-migration shock. Both are retained. Calibration must not force the O-D cells to those incompatible margins.

- PASS: interstate age-sex detail compared with every state-year margin
- PASS: interstate age-sex detail sums to separately rounded margins within 100 persons [worst 65]

The age-sex interstate margins cover the whole window and are independently rounded by cell. They provide age-profile evidence without pretending that age is observed jointly with O-D.

- PASS: net interstate migration across the eight states closes to within 200 persons a year [worst 22]
- PASS: arrivals minus departures equals net interstate migration for every state-year [worst 0]

Every interstate move is one region's arrival and another's departure, so net interstate migration sums to zero across *all* regions. It does not sum to exactly zero across the eight states alone, because Other Territories also exchange population with them and are not published as a separate series in this table. The residual below is that exchange plus rounding.

| run year | net interstate migration, eight states |
|---|---:|
| 2010 | +10 |
| 2011 | +2 |
| 2012 | -1 |
| 2013 | +0 |
| 2014 | -1 |
| 2015 | -22 |
| 2016 | +0 |
| 2017 | +0 |
| 2018 | +0 |
| 2019 | +0 |
| 2020 | +0 |
| 2021 | +0 |
| 2022 | +0 |
| 2023 | +0 |
| 2024 | +0 |

This is the published counterpart of the exact-conservation invariant the model asserts under N1. The model is closed over the eight states, so its own interstate flows must cancel exactly; the small published residual is the price of Other Territories being out of scope.

## Overseas migration margins

Arrivals and departures come from the Overseas Migration release, while net overseas migration comes from the ERP components table. They are independently published and independently rounded, so arrivals minus departures should track published net overseas migration closely without matching exactly.

- PASS: overseas margins compared against published net
- PASS: for settled run years to 2022, implied net overseas migration matches the published figure to within 1,000 persons [worst 84]

| run year | worst absolute difference across states |
|---|---:|
| 2010 | 8 |
| 2011 | 8 |
| 2012 | 6 |
| 2013 | 6 |
| 2014 | 9 |
| 2015 | 7 |
| 2016 | 8 |
| 2017 | 8 |
| 2018 | 9 |
| 2019 | 27 |
| 2020 | 84 |
| 2021 | 8 |
| 2022 | 8 |
| 2023 | 7 |
| 2024 | 3,297 |

The final run years diverge more because the most recent financial year is preliminary in both releases and is revised on different schedules. Treat those years as provisional rather than settled evidence.

- PASS: age-sex overseas detail compared with every state-year margin
- PASS: age-sex overseas detail sums to independently extracted gross margins within cell-rounding tolerance [worst 50]

The detailed SDMX cells and the workbook margins are separate physical sources from the same release. Each detailed cell is rounded to the nearest 10, so summing 28 age-sex cells need not equal the separately rounded margin; the observed worst difference is 50 persons.

## Birth and death registration data

- PASS: registered births cover calendar years 2010-2024
- PASS: national male and female births cover calendar years 2010-2024
- PASS: registered deaths by age and sex cover calendar years 2010-2024
- PASS: birth and death extracts contain no negative counts

- PASS: national sex totals exceed the eight-state birth sum only by the published residual [worst 44]
| calendar year | births, eight states | male births, Australia | female births, Australia | national residual | deaths, eight states | deaths with age not stated |
|---|---:|---:|---:|---:|---:|---:|
| 2010 | 303,299 | 155,591 | 147,727 | 19 | 143,451 | 9 |
| 2011 | 301,585 | 154,996 | 146,621 | 32 | 146,915 | 7 |
| 2012 | 309,543 | 158,988 | 150,594 | 39 | 147,078 | 6 |
| 2013 | 308,038 | 158,706 | 149,359 | 27 | 147,671 | 17 |
| 2014 | 299,664 | 153,592 | 146,105 | 33 | 153,567 | 34 |
| 2015 | 305,340 | 157,088 | 148,289 | 37 | 159,047 | 2 |
| 2016 | 311,064 | 159,537 | 151,567 | 40 | 158,503 | 4 |
| 2017 | 309,112 | 159,221 | 149,921 | 30 | 160,870 | 8 |
| 2018 | 315,103 | 162,088 | 153,059 | 44 | 158,470 | 2 |
| 2019 | 305,809 | 157,476 | 148,356 | 23 | 169,264 | 1 |
| 2020 | 294,344 | 150,943 | 143,426 | 25 | 161,295 | 14 |
| 2021 | 309,963 | 158,917 | 151,079 | 33 | 171,453 | 12 |
| 2022 | 300,651 | 154,281 | 146,403 | 33 | 190,918 | 7 |
| 2023 | 286,964 | 147,422 | 139,576 | 34 | 183,097 | 1 |
| 2024 | 292,294 | 150,299 | 142,019 | 24 | 187,222 | 8 |

These are final **calendar registration-year** counts from Births, Australia and Deaths, Australia. The national male/female series sets the entrant sex ratio; the eight-state residual is retained. These counts are direct evidence, but they are not the same temporal quantity as the financial-year natural-increase component. The pipeline preserves both and does not force one to equal the other.

## Annual age-specific mortality rates

- PASS: mortality-rate extract carries the full 2010-2024 state-sex-band grid
- PASS: all published mortality rates are non-negative
- PASS: the only unpublished rates are NT female 100+ in 2010 and 2011 (zero published exposure)

Rates are ABS registered deaths per 1,000 population in non-overlapping five-year age bands. They are the direct annual mortality input selected in DECISIONS.md §N6. The two absent terminal-band cells remain blank and explicitly flagged; PRD 0005 uses the same-year Australian female 100+ rate rather than treating a missing rate as zero.

## Life-table validation snapshots

- PASS: life-table validation covers the five published XLSX periods from 2018-2020 through 2022-2024
- PASS: each life-table period has all eight states, both sexes and ages 0-100
- PASS: all published qx values lie in [0,1]

These are overlapping three-year **period** life tables. Their period boundaries are retained in the extract; no snapshot is relabelled as an annual observation. Under §N6 they validate the level and age shape of the independently published annual death rates but do not supply model parameters.

## Remaining coverage limitation

Recorded rather than worked around; see `sources.json`:

- **Birth/death temporal basis.** Direct state birth counts and state-age-sex death counts now cover 2010-2024, but they are final calendar registration-year observations. ERP natural increase is a financial-year component based on occurrence and demographic adjustment. Applying the registration series to model run years therefore needs an explicit, documented convention in PRD 0005; this acquisition PRD does not shift or scale the observations.

