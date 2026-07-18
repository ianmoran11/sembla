#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
artifact_dir="${SEMBLA_NPE_ARTIFACT_DIR:-$script_dir/artifacts}"
draws="${SEMBLA_NPE_DRAWS:-2300}"
population="${SEMBLA_NPE_POPULATION:-10000}"
ticks="${SEMBLA_NPE_TICKS:-50}"
training_seed="${SEMBLA_NPE_TRAINING_SEED:-240701}"
heldout_seed="${SEMBLA_NPE_HELDOUT_SEED:-240702}"

if (( draws < 2300 )); then
  echo "error: SEMBLA_NPE_DRAWS must be at least 2300 (2200 train + 100 SBC)" >&2
  exit 2
fi
if [[ -z "$artifact_dir" || "$artifact_dir" == "/" ]]; then
  echo "error: refusing unsafe artifact directory '$artifact_dir'" >&2
  exit 2
fi

mkdir -p "$artifact_dir"
rm -rf "$artifact_dir/training-sweep" "$artifact_dir/heldout-sweep"
rm -f "$artifact_dir/training-pairs.csv" "$artifact_dir/training-pairs.csv.meta.json"
rm -f "$artifact_dir/heldout-pairs.csv" "$artifact_dir/heldout-pairs.csv.meta.json"

cd "$repo_root"
cargo build --release -p sembla-cli
sembla="$repo_root/target/release/sembla"

"$sembla" synth-pop \
  --persons "$population" --employers 250 --initial-infected 100 \
  --seed 8675309 --out "$artifact_dir/population.bin"

"$sembla" sweep examples/sir.json \
  --population "$artifact_dir/population.bin" \
  --seed "$training_seed" --draws "$draws" --ticks "$ticks" \
  --noise independent --out "$artifact_dir/training-sweep" \
  --export-pairs "$artifact_dir/training-pairs.csv"

# The held-out θ* is beta=0.8, gamma=0.1. Its simulation seed is distinct from
# the training sweep's seed, and its observation is another PRD-0006 artifact.
printf '[{"beta":0.8,"gamma":0.1}]\n' > "$artifact_dir/heldout-theta.json"
"$sembla" sweep examples/sir.json \
  --population "$artifact_dir/population.bin" \
  --seed "$heldout_seed" --theta-file "$artifact_dir/heldout-theta.json" \
  --ticks "$ticks" --noise independent --out "$artifact_dir/heldout-sweep" \
  --export-pairs "$artifact_dir/heldout-pairs.csv"

echo "NPE training pairs: $artifact_dir/training-pairs.csv"
echo "held-out observation: $artifact_dir/heldout-pairs.csv"
