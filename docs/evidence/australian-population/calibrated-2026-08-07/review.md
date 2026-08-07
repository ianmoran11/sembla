## Review

### Requested fixes

- **Correct — undefined `theta_file` fixed.** `data/abs/calibrate.py:418-448` now uses the `parameter_digest` supplied by `run_sweep()` rather than reading an out-of-scope variable. The regression at `data/abs/tests/test_calibrate.py:111-149` directly executes `_run_sweep_cpu()` and verifies the supplied digest is retained in the manifest.
- **Correct — strict point-predictive gate implemented exactly as selected.** `data/abs/calibrate.py:741-790` reads only `metrics.by_role.fitted`, requires both MAE and RMSE to satisfy strict `<`, has no tolerance, and excludes held-out metrics by construction. Missing, Boolean, and non-finite values are rejected.
- **Correct — gate is consequential.** `_run_calibrated_year()` aborts on failure at `data/abs/calibrate.py:837-850`; evidence packaging independently rechecks and aborts at `data/abs/calibrate.py:1063-1073`.
- **Correct — regression coverage.** `data/abs/tests/test_calibrate.py:314-359` covers both-metrics success, MAE equality failure, RMSE degradation failure, missing metrics, and non-finite metrics.
- **Correct — all fifteen immutable reports pass.** Independent parsing confirmed years 2010–2024 each have candidate fitted MAE and RMSE strictly below the same-year frozen PRD 0007 baseline. This agrees with `point-predictive-check.json` and `held-out-comparison.json`.
- **Correct — scientific outputs remain unchanged.** The new evidence explicitly records retrospective application to immutable score reports. `chain-verification.json` and the evidence README retain the original parameter, seed, run, score, and state hashes. No parameter or residual report failed checksum verification.

### PRD 0008 acceptance criteria

1. **Satisfied.** `git diff --check` passes. The supplied final `check-abs-data` result is 181 passing tests; the focused calibration suite independently passed 19/19. The complete `scripts/check.sh` result predates these Python/documentation-only fixes.
2. **Satisfied.** Gravity-fit evidence reports all 56 O–D cells, 41 residual degrees of freedom, standalone improvement, residuals, and the unreconciled validation-only 2020 margin conflict.
3. **Satisfied.** `parameter-verification.json` covers all fifteen 377-value files and verifies the seventeen free parameters while the other 360 remain fixed.
4. **Satisfied.** Diagnostics and posterior evidence cover every run year from 2010 through 2024.
5. **Satisfied.** SBC, point-predictive, contraction, admissibility, capacity, and gravity diagnostics are retained. Unidentified parameters are named per year; failed or inadmissible posteriors are disclosed rather than presented as successful fits.
6. **Satisfied.** Every year is O–D-fitted; residual evidence evaluates the gravity form, and the 2020 margin conflict remains validation-only.
7. **Satisfied.** `held-out-comparison.json` computes the baseline comparison solely from held-out `stock_single_year` targets and reports the observed improvement without using held-out cells in the new fitted-role gate.
8. **Satisfied.** `calibration/npe` has no working-tree differences from `HEAD`; no Australian-specific integration was found. The quarantine remains intact.

### Evidence integrity, honesty, and scope

- **Correct:** Every entry in `docs/evidence/australian-population/calibrated-2026-08-07/SHA256SUMS` verified.
- **Correct:** Documentation discloses that only three posteriors were accepted, six failed SBC, six were inadmissible, and calibrated O–D WAPE is worse than gravity-only. It also explicitly states that the point-predictive rule was introduced during final review rather than predeclared.
- **Correct:** Changes remain within the calibration harness, tests, parameter/evidence outputs, guide, and the decision record needed for the explicitly selected rule. No runtime, model, CLI, dependency, lockfile, or quarantined-NPE changes are present.
- **Correct:** `git diff --cached --name-only` was empty.
- **Note:** Because the retained evidence directories are untracked, Git cannot independently compare them with a pre-fix revision. The no-output-change conclusion is supported by their internal raw hashes, retained provenance, and successful checksum inventory rather than Git history.
- **Blocker:** None.

```json
{
  "decision": "approve",
  "blocker_count": 0,
  "high_count": 0,
  "medium_count": 0,
  "low_count": 0,
  "residual_risks": [
    "The untracked immutable evidence has no Git parent for a direct before/after comparison; integrity is instead supported by retained raw hashes and SHA256SUMS."
  ]
}
```