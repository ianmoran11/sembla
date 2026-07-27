# Guarded racing-clock evidence — 2026-07-27

## Scope implemented

Direct Real-literal and Real-parameter hazards are the deliberately narrow
row-invariant fragment. For each such transition, each tick computes:

```text
threshold = exp(-(lambda * dt))
lo = threshold * (1 - 1e-12)
```

Each enabled positive-hazard candidate still converts its row to `entity_id`
first. It then draws the canonical open uniform at the unchanged Philox
coordinates. A draw below `lo` is provably a non-firer and skips `ln`; every
other draw goes through the unchanged `-uniform.ln() / lambda` transform and the
unchanged `race_time < dt` comparison. Row-dependent hazard expressions retain
the unfiltered oracle path.

The tiled evaluator stores one filter in each prepared transition, shared by all
of its fixed tile tasks. The column fallback computes the same one filter after
eager guard, hazard, and contest-column evaluation. Both call the same candidate
helper, preserving error timing and the `EntityIdOverflow` diagnostic.

`rng.rs` splits the existing transform into `exp_f64_from_uniform` so filtering
uses one Philox draw and does not duplicate the exempt platform `ln`. `exp_f64`
still calls the same uniform construction and the exact same transform. A unit
test compares their result bits over representative coordinates and rates.

## Conservative margin argument

At the benchmark thresholds, close to one, one binary64 ULP is at most `2^-52`
relative. The named `1e-12` margin is about 4,500 ULPs. It therefore dominates
the documented one-ULP macOS/Linux transcendental disagreement and the handful
of binary64 rounding steps used to form `lambda * dt`, `exp`, and `lo`. The
threshold is only a rejection filter: all candidates in this wide boundary
envelope still use the platform's canonical `ln` and `< dt` decision.

The implementation deliberately does **not** special-case the two `1e300`
hazards. Their threshold and `lo` are zero, so the open uniform is always
admitted and the exact race time is still computed. This is the conservative
choice for arbitrary future contested models: no draw whose value might be
consumed is removed. It also leaves §E1 coordinate purity trivially unchanged.

## Tests

Tests establish that:

- a value immediately below `lo` is rejected while `lo` and the next value are
  admitted;
- the guarded and oracle firing sets and firing race-time bits match over 100,000
  coordinates for each hazard `0.001`, `0.002`, `0.0025`, `0.003`, `0.012`,
  `0.018`, `0.020`, `0.025`, and `1e300`;
- grouping those candidates onto 128 contested resources produces the same
  race-time argmin winner and exact winning bits;
- a filter that would reject every draw cannot hide `EntityIdOverflow`, because
  row conversion precedes filtering;
- the split uniform/transform path is bit-identical to `exp_f64`.

The existing tiled worker/tile determinism matrix and full oracle/golden suite
remain part of the locked test run.

## Per-transition `ln` fraction

A measurement-only build counted enabled candidates and filter admissions during
one binding 24-tick run. It duplicated the counter-based uniform draw only for
counting; it did not change candidate decisions. That instrumentation was not
used for timings and was fully reverted before the final release binary was
rebuilt and re-hashed.

| Rule | Transition | Enabled candidates | `ln` computed | Fraction |
|---:|---|---:|---:|---:|
| 0 | `age_monthly` | 18,547,052 | 18,547,052 | 100.000000% |
| 1 | `clear_event` | 279,009 | 279,009 | 100.000000% |
| 2 | `birth_activate` | 3,721,059 | 91,823 | 2.467658% |
| 3 | `overseas_arrive` | 972,512 | 19,280 | 1.982495% |
| 4 | `internal_arrive` | 596,491 | 10,560 | 1.770354% |
| 5 | `die_young` | 4,700,929 | 4,708 | 0.100150% |
| 6 | `die_adult` | 9,081,191 | 27,148 | 0.298948% |
| 7 | `die_old` | 4,648,809 | 55,388 | 1.191445% |
| 8 | `emigrate` | 18,430,929 | 36,962 | 0.200543% |
| 9 | `internal_depart` | 18,430,929 | 45,819 | 0.248598% |

The two degenerate transitions are intentionally 100%. All eight ordinary
benchmark hazards compute `ln` for only 0.10%–2.47% of enabled candidates, close
to their actual fire probabilities and nowhere near the rejected 100% shape.

## Official five-run protocol

The fixed case is the no-grouped demographic model with 1,000,000 slots, 24
ticks, seed 9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. Measurements were
made in-run on an Apple M2 Pro with 10 physical/logical cores and 16 GiB RAM;
`available_parallelism()` reported 10.

Each run used:

```sh
/usr/bin/time -l -o <time-file> <binary> run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <run-output.csv>
```

A run is contended when `wall - (user + sys) > 0.5 s`. No official run was
contended. Negative gaps are expected because user time sums CPU consumption
across tile workers.

| Build | Run | Wall s | User s | Sys s | Gap s | Contended |
|---|---:|---:|---:|---:|---:|:---:|
| Before guarded filter | 1 | 6.12 | 6.82 | 1.38 | -2.08 | no |
| Before guarded filter | 2 | 6.40 | 7.07 | 1.55 | -2.22 | no |
| Before guarded filter | 3 | 6.03 | 6.99 | 1.48 | -2.44 | no |
| Before guarded filter | 4 | 5.74 | 6.81 | 1.46 | -2.53 | no |
| Before guarded filter | 5 | **5.34** | **6.66** | 1.20 | -2.52 | no |
| After guarded filter | 1 | 5.87 | 6.03 | 1.38 | -1.54 | no |
| After guarded filter | 2 | 5.44 | 5.97 | 1.31 | -1.84 | no |
| After guarded filter | 3 | 5.57 | 6.00 | 1.40 | -1.83 | no |
| After guarded filter | 4 | **5.35** | **5.95** | 1.25 | -1.85 | no |
| After guarded filter | 5 | 5.49 | 5.98 | 1.35 | -1.84 | no |

Fastest uncontended user time improved from 6.66 s to 5.95 s, a **1.119x
speedup** and 10.66% reduction. Corresponding wall time was essentially flat,
changing from 5.34 s to 5.35 s (0.998x). Medians were 6.03 / 6.82 / 1.46
seconds before and 5.49 / 5.98 / 1.35 seconds after (wall / user / sys).

## Identity

- baseline commit: `c77a1c0`;
- before binary SHA-256:
  `5b93233b42429da08021eb8eb2945465505f5ca13ee0be680437da6616b86fb8`;
- final after binary SHA-256:
  `ef2e41667dee917a1fe2164b160bb6b455cb53c05ba86f6c0c4f96c1b6e85350`;
- measurement-only stats binary SHA-256:
  `39e8937ae9b90a1fecd21043bb1b3f90f8bbf111da96b9a18ddd91a7a8bffa32`;
- resized model SHA-256:
  `601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`;
- initial state SHA-256:
  `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`.

All five before, five after, and the stats run matched:

- primary CSV SHA-256:
  `eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`;
- summaries CSV SHA-256:
  `329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`;
- manifest SHA-256:
  `dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`;
- stdout SHA-256:
  `a034ac5ea499bc5ee82b57c93daf43a8d6580a5d98cf1c0757341aa71fc8154f`;
- `final_state_sha256`:
  `2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

## Acceptance gates

Final-workspace gates passed:

- `cargo test --locked`;
- `scripts/check-rust.sh`;
- `python3 scripts/check-markdown-links.py`;
- JSON validation for `measurements.json`;
- `git diff --check`.

No tracked file under `examples/**`, `fixtures/**`, `frontend/Fixtures/**`, or
tracked CUDA differential evidence changed. The full machine-readable runs,
contention flags, medians, fractions, constants, and hashes are in
`measurements.json`.
