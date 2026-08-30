#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <output-directory>" >&2
    exit 2
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
output_root="$1"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/sembla-frontend-data.XXXXXX")"
trap 'rm -rf "$temporary_root"' EXIT

tables="$output_root/Sembla/Models/AustralianPopulation/Data"
reference="$output_root/Sembla/TestData/AustralianPopulation/AustralianPopulationParameters.lean"

python3 "$repo_root/data/abs/rates.py" \
    --params-dir "$temporary_root/params" \
    --report "$temporary_root/rates.md" \
    --parameter-tables "$tables" \
    --lean-parameters "$reference" \
    >/dev/null

(
    cd "$output_root"
    shasum -a 256 \
        Sembla/Models/AustralianPopulation/Data/birth_rate.json \
        Sembla/Models/AustralianPopulation/Data/emigration.json \
        Sembla/Models/AustralianPopulation/Data/mortality.json \
        Sembla/Models/AustralianPopulation/Data/overseas_arrival.json \
        Sembla/TestData/AustralianPopulation/AustralianPopulationParameters.lean \
        > frontend-data.sha256
)

echo "exported deterministic Lean frontend data to $output_root"
