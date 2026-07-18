# NPE reference pipeline

This directory is the quarantined external calibration workflow from
`DECISIONS.md` §G5. It consumes PRD-0006 artifacts and never imports a Sembla
crate, Rust library, model parser, or runtime API. It is intentionally outside
the Cargo workspace, CPU-only, and is not a dependency of production code.

## Pinned CPU environment

The reference is tested with Python 3.12. Create an isolated environment from
the repository root:

```sh
python3.12 -m venv calibration/npe/.venv
. calibration/npe/.venv/bin/activate
python -m pip install --upgrade pip
python -m pip install -r calibration/npe/requirements.txt
```

`requirements.txt` pins `sbi`, the platform's CPU build of `torch`, `numpy`,
`pandas`, `scipy`, and `pytest` exactly. No CUDA package is used.

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
