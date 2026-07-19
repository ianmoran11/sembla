#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
if ! git diff --quiet || ! git diff --cached --quiet || [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
  echo "error: differential evidence requires a clean committed worktree" >&2
  exit 1
fi
command -v nvidia-smi >/dev/null || { echo "error: nvidia-smi is required; result remains unanswered" >&2; exit 1; }
command -v nvcc >/dev/null || { echo "error: nvcc is required; result remains unanswered" >&2; exit 1; }
if command -v ldconfig >/dev/null 2>&1; then
  nvrtc_libraries="$(ldconfig -p 2>/dev/null || true)"
  grep -q 'libnvrtc' <<<"$nvrtc_libraries" || {
    echo "error: NVRTC shared library is required; result remains unanswered" >&2
    exit 1
  }
fi

stamp="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
out="${SEMBLA_CUDA_EVIDENCE_DIR:-target/sembla-differential-evidence/$stamp}"
mkdir -p "$out"
{
  echo "commit=$(git rev-parse HEAD)"
  echo "utc=$stamp"
  echo "driver=$(nvidia-smi --query-gpu=driver_version --format=csv,noheader | head -1)"
  echo "gpu=$(nvidia-smi --query-gpu=name --format=csv,noheader | head -1)"
  echo "correctness_hardware=any CUDA-capable NVIDIA GPU"
  echo "performance_hardware=verified full-rate FP64 required"
} | tee "$out/provenance.txt"

set +e
cargo test --locked --release -p sembla-cli --features cuda --test gpu_differential -- --ignored --nocapture 2>&1 | tee "$out/tests.log"
status=${PIPESTATUS[0]}
if [[ $status -eq 0 ]]; then
  cargo run --locked --release -p sembla-cli --features cuda -- diff-backends \
    --all-examples --population 100 --seed 7 --ticks 20 \
    2>&1 | tee "$out/corpus.log"
  status=${PIPESTATUS[0]}
fi
set -e
if [[ $status -eq 0 && "${SEMBLA_RUN_FULL_RATE:-0}" == "1" ]]; then
  population="$out/full-rate-26m-population.bin"
  set +e
  cargo run --locked --release -p sembla-cli -- synth-pop \
    --persons 26000000 --employers 1300000 --initial-infected 100 \
    --seed 77 --out "$population" 2>&1 | tee "$out/full-rate-population.log"
  population_status=${PIPESTATUS[0]}
  throughput_status=not-run
  if [[ $population_status -eq 0 ]]; then
    cargo run --locked --release -p sembla-cli --features cuda -- diff-backends \
      examples/sir.json --population "$population" --seed 77 --ticks 1 \
      2>&1 | tee "$out/full-rate-26m.log"
    throughput_status=${PIPESTATUS[0]}
  fi
  set -e
  {
    echo "full_rate_population_status=$population_status"
    echo "full_rate_throughput_status=$throughput_status"
  } | tee -a "$out/provenance.txt"
  rm -f "$population"
fi
evidence_files=(provenance.txt tests.log)
[[ -f "$out/corpus.log" ]] && evidence_files+=(corpus.log)
[[ -f "$out/full-rate-population.log" ]] && evidence_files+=(full-rate-population.log)
[[ -f "$out/full-rate-26m.log" ]] && evidence_files+=(full-rate-26m.log)
if command -v sha256sum >/dev/null; then
  (cd "$out" && sha256sum "${evidence_files[@]}" > SHA256SUMS)
else
  (cd "$out" && shasum -a 256 "${evidence_files[@]}" > SHA256SUMS)
fi
cat "$out/SHA256SUMS"
if [[ $status -ne 0 ]]; then
  echo "CUDA differential corpus failed; evidence: $out" >&2
  exit "$status"
fi
echo "CUDA differential corpus passed; evidence: $out"
echo "Copy the recorded commit/GPU/driver/verdict/rates into the dated evidence note, then destroy remote resources."

