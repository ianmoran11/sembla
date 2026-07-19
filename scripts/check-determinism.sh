#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

cargo build --quiet --locked -p sembla-cli

target_dir="${CARGO_TARGET_DIR:-target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$repo_root/$target_dir"
fi
sembla="$target_dir/debug/sembla"
if [[ ! -x "$sembla" ]]; then
  echo "error: expected Sembla binary at $sembla" >&2
  exit 1
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/sembla-determinism.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

compare() {
  local label="$1" first="$2" second="$3"
  if ! diff -u "$first" "$second"; then
    echo "error: deterministic $label differs byte-for-byte" >&2
    exit 1
  fi
}

for repeat in first second; do
  "$sembla" run examples/sir.json \
    --backend cpu \
    --population 1000 \
    --seed 240710 \
    --ticks 12 \
    --out "$tmp/run-$repeat.csv" \
    >"$tmp/run-$repeat.stdout"
done
compare "run results" "$tmp/run-first.csv" "$tmp/run-second.csv"
compare "run summaries" \
  "$tmp/run-first.csv.summaries.csv" \
  "$tmp/run-second.csv.summaries.csv"
compare "run manifest" \
  "$tmp/run-first.csv.manifest.json" \
  "$tmp/run-second.csv.manifest.json"

for repeat in first second; do
  "$sembla" sweep examples/sir.json \
    --backend cpu \
    --population 256 \
    --seed 240711 \
    --draws 3 \
    --ticks 8 \
    --noise independent \
    --out "$tmp/sweep-$repeat" \
    >"$tmp/sweep-$repeat.stdout"
done
if ! diff -ru "$tmp/sweep-first" "$tmp/sweep-second"; then
  echo "error: deterministic sweep outputs differ byte-for-byte" >&2
  exit 1
fi
compare "sweep manifest" \
  "$tmp/sweep-first/run-manifest.json" \
  "$tmp/sweep-second/run-manifest.json"

echo "determinism checks passed: run results/summaries/manifest and sweep outputs/manifest are byte-identical"
