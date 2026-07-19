#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"
python_bin="${PYTHON:-python}"

"$python_bin" -m pytest \
  calibration/npe/tests/test_contract.py \
  calibration/npe/tests/test_quarantine.py

tmp="$(mktemp -d "${TMPDIR:-/tmp}/sembla-npe-smoke.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
fixture_dir="calibration/npe/tests/fixtures/smoke"

# Exercise the actual sbi/Torch training API with the smallest useful split.
# The 100 held-back rows satisfy train.py's SBC contract, but this smoke test
# deliberately does not invoke sbc.py; full statistical acceptance stays manual.
"$python_bin" calibration/npe/train.py \
  --pairs "$fixture_dir/training-pairs.csv" \
  --observation "$fixture_dir/heldout-pairs.csv" \
  --output "$tmp/run" \
  --train-draws 16 \
  --sbc-draws 100 \
  --posterior-samples 8 \
  --sbc-posterior-samples 8 \
  --batch-size 8 \
  --hidden-features 8 \
  --num-transforms 1 \
  --stop-after-epochs 1 \
  --max-num-epochs 1

for artifact in diagnostics.json posterior-samples.csv posterior.pt; do
  if [[ ! -s "$tmp/run/$artifact" ]]; then
    echo "error: NPE smoke training did not produce $artifact" >&2
    exit 1
  fi
done

"$python_bin" - "$tmp/run/diagnostics.json" <<'PY'
import json
import pathlib
import sys

diagnostics = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
assert diagnostics["training"]["draws"] == 16
assert diagnostics["training"]["max_num_epochs"] == 1
assert diagnostics["sbc"]["status"] == "pending"
PY

echo "NPE smoke checks passed: contract refusals and reduced training (SBC intentionally not run)"
