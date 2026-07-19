#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
fixture_dir="$script_dir/tests/fixtures/smoke"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/sembla-npe-fixtures.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

cd "$repo_root"
cargo build --release -p sembla-cli
sembla="$repo_root/target/release/sembla"

"$sembla" synth-pop \
  --persons 10000 --employers 250 --initial-infected 100 \
  --seed 8675309 --out "$tmp/population.bin"

"$sembla" sweep examples/sir.json \
  --population "$tmp/population.bin" \
  --seed 240701 --draws 116 --ticks 50 \
  --noise independent --out "$tmp/training-sweep" \
  --export-pairs "$tmp/training-pairs.csv"

printf '[{"beta":0.8,"gamma":0.1}]\n' > "$tmp/heldout-theta.json"
"$sembla" sweep examples/sir.json \
  --population "$tmp/population.bin" \
  --seed 240702 --theta-file "$tmp/heldout-theta.json" \
  --ticks 50 --noise independent --out "$tmp/heldout-sweep" \
  --export-pairs "$tmp/heldout-pairs.csv"

mkdir -p "$fixture_dir"
install -m 0644 "$tmp/training-pairs.csv" "$fixture_dir/training-pairs.csv"
install -m 0644 "$tmp/training-pairs.csv.meta.json" \
  "$fixture_dir/training-pairs.csv.meta.json"
install -m 0644 "$tmp/heldout-pairs.csv" "$fixture_dir/heldout-pairs.csv"
install -m 0644 "$tmp/heldout-pairs.csv.meta.json" \
  "$fixture_dir/heldout-pairs.csv.meta.json"

echo "regenerated NPE smoke fixtures with exporter-produced metadata"
