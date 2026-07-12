#!/usr/bin/env bash
set -euo pipefail
frontend_root="$(cd "$(dirname "$0")/.." && pwd)"
repo_root="$(cd "$frontend_root/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/sembla-lean.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

cd "$frontend_root"
lake build
bash scripts/test-negative.sh
lake exe sembla-export sir "$tmp/sir.json"
lake exe sembla-export Sembla.Models.sirPolicy "$tmp/sir_policy.json"

cd "$repo_root"
cargo build --quiet -p sembla-cli
sembla="$repo_root/target/debug/sembla"
"$sembla" validate "$tmp/sir.json"
"$sembla" validate "$tmp/sir_policy.json"
"$sembla" diff-ir examples/sir.json "$tmp/sir.json"
"$sembla" diff-ir examples/sir_policy.json "$tmp/sir_policy.json"

# End-to-end parity traverses population serialization and the executor, not
# only JSON normalization. Identical stdout proves both results and final-state
# hashes match; byte-identical CSV is an additional assertion.
"$sembla" synth-pop --persons 1000 --employers 50 --initial-infected 10 --seed 12 --out "$tmp/pop.bin" >/dev/null
"$sembla" run examples/sir.json --population "$tmp/pop.bin" --seed 55 --ticks 20 --out "$tmp/fixture.csv" >"$tmp/fixture.hashes"
"$sembla" run "$tmp/sir.json" --population "$tmp/pop.bin" --seed 55 --ticks 20 --out "$tmp/exported.csv" >"$tmp/exported.hashes"
cmp "$tmp/fixture.hashes" "$tmp/exported.hashes"
cmp "$tmp/fixture.csv" "$tmp/exported.csv"
echo "Lean export, validation, normalized parity, and run-hash parity passed"
