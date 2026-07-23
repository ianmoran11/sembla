# NPE reference pipeline

This directory is the quarantined external calibration workflow from
`DECISIONS.md` §G5. It consumes PRD-0006 artifacts and never imports a Sembla
crate, Rust library, model parser, or runtime API. It is intentionally outside
the Cargo workspace, CPU-only, and is not a dependency of production code.

## Pinned CPU environment

`requirements.txt` remains the concise, human-maintained input. It pins `sbi`,
the platform's CPU build of `torch`, `numpy`, `pandas`, `scipy`, and `pytest`
exactly. PyPI is the only package index; the torch-only page under
`https://download.pytorch.org/whl/cpu/torch/` is a `--find-links` source rather
than a competing package index. No CUDA package is used.

The reviewed CI environment is `requirements-ci.lock`. It contains the complete
Linux/amd64 CPython 3.12.8 graph with exact versions and SHA-256 hashes. CI and
Linux validation install it through the repository script so the sdist-only
`nflows==0.14` cannot create an unconstrained PEP 517 build environment:

```sh
python3.12 -m venv calibration/npe/.venv
PYTHON=calibration/npe/.venv/bin/python \
  calibration/npe/install-requirements-ci.sh
```

The installer first installs the lock's hashed `setuptools==75.8.0` and
`wheel==0.45.1` entries without dependencies. It then installs the same lock
with both `--require-hashes` and `--no-build-isolation`. It never resolves
`requirements.txt`. The direct input may still be used for a local Darwin setup,
which is intentionally outside the Linux CI lock contract:

```sh
python3.12 -m venv calibration/npe/.venv
. calibration/npe/.venv/bin/activate
python -m pip install -r calibration/npe/requirements.txt
```

### Lock regeneration and authoritative validation

Run this exact command from the repository root:

```sh
./scripts/check-npe-lock.sh
```

It runs Docker with `--platform linux/amd64` and the immutable platform image:

```text
python:3.12.8-slim-bookworm@sha256:8859bd6ca943079262c27e38b7119cdacede77c463139a15651dd340087a6cc9
```

The repository is mounted read-only. Inside that image the command installs only
the pinned `uv==0.5.18` manylinux wheel with SHA-256
`04e6c62d8947f62f1ec3255b5743cc775950b6203b06bf9c4d50682dcd68f340`
into a temporary generator environment. It resolves for CPython 3.12 and
`x86_64-manylinux_2_17`, uses a fixed `2026-07-23T00:00:00Z` release cutoff,
and writes through a temporary file. The direct input SHA-256 and the generator,
container, target, source, and cutoff contracts are recorded in the lock header.

`uv` 0.5.18 does not emit a hash for a package selected from a flat
`--find-links` page. The generator therefore requires exactly one
`torch==2.5.1+cpu` result and attaches the independently verified Linux CPython
3.12 wheel hash
`4856f9d6925121d13c2df07aa7580b767f449dfe71ae5acde9c27535d5da4840`.
The subsequent pip installation verifies that hash against the downloaded
PyTorch wheel.

Validation regenerates to a temporary path, byte-compares the checked lock,
creates a fresh virtual environment, installs with `--require-hashes`, rejects
any isolated build-dependency resolution, and runs
`./scripts/check-npe-smoke.sh` in the same pinned container. A missing or failed
Docker run is a failed validation, not a deferred CI claim.

If those exact dependencies cannot be installed, the reference tests write
`calibration/npe/artifacts/run/diagnostics.json` with `status: "unanswered"`,
`pass: false`, and the dependency or artifact reason, then skip the statistical
tests. That is an unanswered environment, never evidence of a pass. The
standard-library contract and quarantine tests still run.

## Reference configuration

`generate_data.sh` builds the CLI and generates two ordinary PRD-0006 exports:

- 2,300 independent-noise SIR draws at population 10,000 and 50 ticks. The
  first 2,200 train the amortized posterior and the final 100 are reserved for
  SBC.
- one held-out observation at **θ\* = (`beta` 0.8, `gamma` 0.1)** with master
  seed 240702, distinct from training seed 240701.

The defaults fit a laptop CPU while retaining the PRD's 2,000–5,000 training
range. `SEMBLA_NPE_ARTIFACT_DIR`, `SEMBLA_NPE_DRAWS` (at least 2,300),
`SEMBLA_NPE_POPULATION`, and `SEMBLA_NPE_TICKS` may relocate or enlarge a run;
the values above are the acceptance configuration.

From the repository root, the complete acceptance loop is:

```sh
. calibration/npe/.venv/bin/activate
bash calibration/npe/generate_data.sh
python -m pytest calibration/npe/tests
```

The reduced checked-in smoke fixtures are regenerated separately, through the
same CLI exporter so their `pairs_sha256` sidecars are never hand-edited:

```sh
bash calibration/npe/regenerate_smoke_fixtures.sh
```

The curated committed reference evidence and the portable-sampler regeneration
record are described in `calibration/npe/artifacts/README.md`. Raw per-draw
sweeps, the population binary, and timestamped training logs remain reproducible
intermediates and are ignored.

The reference tests run the same training and SBC pipeline twice. The primary
outputs are under `calibration/npe/artifacts/run/`; the repeated run is under
`artifacts/repro-run/`. For a single manual run, the equivalent commands are:

```sh
python calibration/npe/train.py
python calibration/npe/sbc.py
```

This is the full flow:

1. `sembla sweep --noise independent --export-pairs` samples θ and simulates x.
2. `train.py` validates the export, reserves SBC rows, and trains one amortized
   `sbi.inference.NPE` round with an NSF normalizing-flow density estimator.
3. The held-out PRD-0006 export supplies x* and its documented θ* only for
   evaluating recovery; it is not part of training.
4. `sbc.py` draws marginal posterior ranks for 100 prior-predictive rows and
   applies per-parameter Kolmogorov–Smirnov tests against uniformity.
5. `pytest` checks recovery, SBC, refusal behavior, reproducibility, and Cargo
   quarantine.

## Input contract and refusals

Each input is exactly a `pairs.csv` and its adjacent `pairs.csv.meta.json`. The
pipeline validates the sidecar before importing `sbi`. It refuses with a named
error when:

- `schema_versions.pairs` is not major 1 (`unsupported pairs schema major`);
- SHA-256 of the exact CSV bytes differs from `pairs_sha256`
  (`pairs_sha256 mismatch`); or
- `noise_mode` is `crn` (`refusing CRN-mode pairs`).

CRN pairs are deliberately invalid for NPE: reusing one simulation-noise
realization makes θ→x artificially deterministic and yields overconfident
posteriors (`DECISIONS.md` §G5). Both the training and held-out observations
must be independent-noise exports with matching effective-IR and column
contracts. No Sembla IR or run-manifest file is read by Python.

## Fixed statistical acceptance thresholds

These thresholds are part of PRD 0007. Changing one requires an explicit PRD
note; do not silently tune a failing run.

### Recovery

For every parameter:

- θ* must lie inside the posterior's **95% marginal credible interval**. This is
  the conventional interval that allows 5% tail risk without making the small
  reference example vacuous.
- The posterior mean must be within an absolute tolerance of θ*: **0.25 for
  `beta` and 0.05 for `gamma`**. These tolerances are approximately one prior
  scale for the checked-in SIR priors, wide enough for finite population and
  summary compression but narrow enough to reject a posterior centered on the
  wrong epidemiological regime.

Both checks must pass for both parameters.

### Simulation-based calibration

There are **100 rank statistics per parameter**, each based on 256 posterior
samples. One hundred is the minimum recommended order for an SBC uniformity
check while remaining practical on a laptop. Each marginal uses a two-sided
Kolmogorov–Smirnov test against the uniform rank distribution and must have
**p > 0.01**. The 1% threshold limits false rejection for this compact
reference while still exposing substantial posterior miscalibration. Every
parameter must pass.

Overall `pass` is true only when all recovery and SBC checks pass.

## Outputs and reproducibility

`train.py` writes:

- `posterior-samples.csv`: 2,000 held-out posterior draws;
- `posterior.pt`: the local `sbi` posterior plus the reserved SBC tensors; and
- `diagnostics.json`, completed by `sbc.py`.

The diagnostics record the SHA-256 hashes of both input CSVs and metadata
sidecars, all random seeds, thread count and flow configuration, marginal
means/median/95% quantiles, true values and tolerances, all SBC ranks and KS
p-values, the recovery/SBC verdicts, and overall `pass`.

NumPy and Torch seeds are fixed and BLAS/Torch thread counts are pinned to one.
Nevertheless, **bit-exact neural training is not claimed**: compiler, CPU
instruction, BLAS, and Torch/sbi kernel differences may perturb optimization.
For two runs from the same artifacts and seeds, tests require identical boolean
verdicts and posterior means/quantiles within absolute tolerance **0.02**; CSV
or JSON byte equality is neither expected nor asserted. Acceptance is
statistical, unlike Sembla's bitwise runtime contract.
