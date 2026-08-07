#!/usr/bin/env bash
# Driver for the PRD 0008 calibration harness.
#
# Stage 1 (always run): the offline Poisson gravity fit, standard library only.
# Stage 2 (optional): the NPE pilot or per-year loop, which requires the pinned
# calibration venv (calibration/npe/.venv) and hours of simulation.
#
# Usage:
#   scripts/calibrate-australian-population.sh                 # stage 1 only
#   scripts/calibrate-australian-population.sh --pilot [YEAR] [DRAWS]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root/data/abs"

echo "== stage 1: offline gravity fit =="
python3 gravity_fit.py

if [[ "${1:-}" != "--pilot" ]]; then
  echo
  echo "stage 2 skipped; pass --pilot [run-year] [draws] for the NPE stage"
  exit 0
fi

run_year="${2:-2010}"
draws="${3:-960}"
venv="$root/calibration/npe/.venv/bin/python"
out="/tmp/australian-npe-pilot-${run_year}"

echo
echo "== stage 2: NPE pilot for ${run_year} with ${draws} draws =="
if [[ ! -x "$venv" ]]; then
  cat >&2 <<'EOF'
NPE environment unavailable: calibration/npe/.venv is missing.
status: "unanswered"; pass: false. Install the pinned environment per
calibration/npe/README.md (an unanswered environment is never a pass).
EOF
  exit 1
fi

python3 calibrate.py pilot \
  --run-year "$run_year" --draws "$draws" \
  --workers 8 --threads 4 \
  --out "$out" --force
