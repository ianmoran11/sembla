#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

if [[ "${1:-}" == "--list" ]]; then
  cat <<'EOF'
diagnostic_case=fixtures/validation-negative/claim_key_overflow.json expected_status=10,2
diagnostic_case=fixtures/validation-negative/transition_guard_overflow.json expected_status=3,2
diagnostic_case=fixtures/validation-negative/effect_int_overflow.json expected_status=5,2
diagnostic_case=fixtures/validation-negative/output_expression_overflow.json expected_status=9,1
corpus_model=fixtures/demographic/benchmark/demographic_slots.no-grouped.json configuration=no-grouped population=1000 seed=7 ticks=20
launch_geometry=1x1
launch_geometry=1x32
launch_geometry=3x4
EOF
  exit 0
fi
if [[ $# -ne 0 ]]; then
  echo "usage: $0 [--list]" >&2
  exit 2
fi

skip_or_fail() {
  local reason="$1"
  if [[ "${SEMBLA_REQUIRE_CUDA:-0}" == "1" ]]; then
    echo "error: $reason; CUDA evidence remains unanswered" >&2
    exit 1
  fi
  echo "SKIP: CUDA differential corpus hardware criteria pending: $reason"
  exit 0
}

command -v nvidia-smi >/dev/null || skip_or_fail "nvidia-smi unavailable"
command -v nvcc >/dev/null || skip_or_fail "nvcc unavailable"
if ! gpu_rows="$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null)" \
  || [[ -z "${gpu_rows//[[:space:]]/}" ]]; then
  skip_or_fail "no usable CUDA device reported by nvidia-smi"
fi
if ! driver_rows="$(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null)" \
  || [[ -z "${driver_rows//[[:space:]]/}" ]]; then
  skip_or_fail "CUDA driver query failed"
fi
gpu="${gpu_rows%%$'\n'*}"
driver="${driver_rows%%$'\n'*}"
if command -v ldconfig >/dev/null 2>&1; then
  nvrtc_libraries="$(ldconfig -p 2>/dev/null || true)"
  grep -q 'libnvrtc' <<<"$nvrtc_libraries" || skip_or_fail "NVRTC shared library unavailable"
fi

# Evidence is meaningful only for the exact committed tree. Listing and local
# GPU-less skip happen before this gate so both remain usable during review.
if ! git diff --quiet || ! git diff --cached --quiet || [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
  echo "error: differential evidence requires a clean committed worktree" >&2
  exit 1
fi

stamp="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
out="${SEMBLA_CUDA_EVIDENCE_DIR:-target/sembla-differential-evidence/$stamp}"
mkdir -p "$out"
{
  echo "commit=$(git rev-parse HEAD)"
  echo "utc=$stamp"
  echo "driver=$driver"
  echo "gpu=$gpu"
  echo "correctness_hardware=any CUDA-capable NVIDIA GPU"
  echo "performance_hardware=verified full-rate FP64 required"
} | tee "$out/provenance.txt"

set +e
cargo test --locked --release -p sembla-cuda --features cuda --lib \
  negative_corpus_matches_cpu_status_under_three_geometries -- --ignored --nocapture \
  2>&1 | tee "$out/diagnostic-corpus.log"
status=${PIPESTATUS[0]}
if [[ $status -eq 0 ]]; then
  cargo run --locked --release -p sembla-cli --features cuda -- diff-backends \
    fixtures/demographic/benchmark/demographic_slots.no-grouped.json \
    --population 1000 --seed 7 --ticks 20 \
    2>&1 | tee "$out/demographic-corpus.log"
  status=${PIPESTATUS[0]}
fi
if [[ $status -eq 0 ]]; then
  cargo test --locked --release -p sembla-cli --features cuda --test gpu_differential -- --ignored --nocapture \
    2>&1 | tee "$out/tests.log"
  status=${PIPESTATUS[0]}
fi
if [[ $status -eq 0 ]]; then
  cargo run --locked --release -p sembla-cli --features cuda -- diff-backends \
    --all-examples --population 100 --seed 7 --ticks 20 \
    2>&1 | tee "$out/corpus.log"
  status=${PIPESTATUS[0]}
fi
if [[ $status -eq 0 ]]; then
  cargo run --locked --release -p sembla-cli --features cuda -- diff-backends \
    --all-plan-fixtures --population 1000 --seed 7 --ticks 20 \
    2>&1 | tee "$out/plan-corpus.log"
  plan_status=${PIPESTATUS[0]}
  grouped_diagnostic="--enable grouped-observations is not yet supported for diff-backends; see the grouped-observations backend follow-up PRD"
  if [[ $plan_status -eq 0 ]]; then
    echo "error: --all-plan-fixtures unexpectedly accepted grouped observations" \
      | tee -a "$out/plan-corpus.log" >&2
    status=1
  elif grep -Fqx -- "$grouped_diagnostic" "$out/plan-corpus.log"; then
    echo "plan_corpus_grouped_rejection=expected" | tee -a "$out/provenance.txt"
    status=0
  else
    status=$plan_status
  fi
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
evidence_files=(provenance.txt)
[[ -f "$out/diagnostic-corpus.log" ]] && evidence_files+=(diagnostic-corpus.log)
[[ -f "$out/tests.log" ]] && evidence_files+=(tests.log)
[[ -f "$out/demographic-corpus.log" ]] && evidence_files+=(demographic-corpus.log)
[[ -f "$out/corpus.log" ]] && evidence_files+=(corpus.log)
[[ -f "$out/plan-corpus.log" ]] && evidence_files+=(plan-corpus.log)
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
