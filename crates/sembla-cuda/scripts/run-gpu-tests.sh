#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if ! git diff --quiet || ! git diff --cached --quiet ||
  [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
  echo "error: GPU evidence requires a clean, committed worktree" >&2
  exit 1
fi

if ! command -v nvidia-smi >/dev/null 2>&1; then
  echo "error: nvidia-smi is required; GPU criteria remain unanswered" >&2
  exit 1
fi
if ! command -v nvcc >/dev/null 2>&1; then
  echo "error: nvcc/toolkit is required to establish CUDA provenance" >&2
  exit 1
fi

# Cudarc loads NVRTC dynamically. This check makes a deliberately requested
# remote run fail rather than silently skipping when the compiler library is absent.
if command -v ldconfig >/dev/null 2>&1; then
  nvrtc_libraries="$(ldconfig -p 2>/dev/null || true)"
  if ! grep -q 'libnvrtc' <<<"$nvrtc_libraries"; then
    echo "error: NVRTC shared library was not found" >&2
    exit 1
  fi
fi

stamp="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
out_dir="${SEMBLA_CUDA_EVIDENCE_DIR:-target/sembla-cuda-evidence/$stamp}"
mkdir -p "$out_dir"

{
  echo "commit=$(git rev-parse HEAD)"
  echo "rustc=$(rustc --version)"
  echo "cargo=$(cargo --version)"
  echo "nvcc=$(nvcc --version | tail -n 1)"
  echo "utc=$stamp"
  echo "correctness_hardware=any compatible CUDA GPU"
  echo "performance_hardware=full-rate GPU required for performance claims"
  echo "--- nvidia-smi ---"
  nvidia-smi --query-gpu=name,uuid,driver_version,compute_cap --format=csv,noheader
} | tee "$out_dir/provenance.txt"

set +e
cargo test --locked --release -p sembla-cuda --features cuda -- \
  --ignored --nocapture 2>&1 | tee "$out_dir/tests.log"
test_status=${PIPESTATUS[0]}
set -e

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$out_dir" && sha256sum provenance.txt tests.log > SHA256SUMS)
else
  (cd "$out_dir" && shasum -a 256 provenance.txt tests.log > SHA256SUMS)
fi
cat "$out_dir/SHA256SUMS"

if [[ $test_status -ne 0 ]]; then
  echo "GPU correctness tests failed; evidence: $out_dir" >&2
  exit "$test_status"
fi

echo "GPU correctness tests passed; evidence: $out_dir"
echo "Follow spikes/precision/infra-hyperstack/README.md and destroy remote resources now."
