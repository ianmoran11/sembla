#!/usr/bin/env bash
# Offline checks for the ABS data pipeline: reader and extract tests, cache
# verification without network access, and extract reconciliation.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root/data/abs"

echo "== extract and reader tests =="
python3 -m unittest discover -s tests -v

echo
echo "== cache verification (no network) =="
python3 fetch.py

echo
echo "== extracts, rates, targets, parameters, and reports regenerate byte-identically =="
artifacts=(extracts/*.csv extracts/*.md params/*.json params/gravity/*.json
  targets/*.json targets/sensitivity/*.json)
before="$(shasum -a 256 "${artifacts[@]}")"
python3 normalise.py >/dev/null
python3 reconcile.py >/dev/null
python3 build_state.py --write-report >/dev/null
python3 rates.py >/dev/null
python3 gravity_fit.py >/dev/null
python3 targets.py >/dev/null
after="$(shasum -a 256 "${artifacts[@]}")"
if [ "$before" != "$after" ]; then
  echo "extracts, rates, targets, parameters, or reports are not byte-reproducible" >&2
  diff <(echo "$before") <(echo "$after") >&2 || true
  exit 1
fi
echo "ok"

echo
echo "== hundredth state artifact regenerates byte-identically =="
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
python3 build_state.py \
  --scale hundredth \
  --model ../../fixtures/australian-population/australian_population.hundredth.json \
  --out "$temporary/initial.state" >/dev/null
cmp "$temporary/initial.state" \
  ../../fixtures/state/australian_population_2010_hundredth.state
cmp "$temporary/initial.state.model.json" \
  ../../fixtures/state/australian_population_2010_hundredth.state.model.json
echo "ok"
