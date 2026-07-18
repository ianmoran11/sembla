#!/usr/bin/env bash
set -euo pipefail

# shellcheck disable=SC1091
source /etc/sembla-spike.env

: "${SPIKE_DIR:?cloud-init did not set SPIKE_DIR}"
: "${VULTR_REGION:?cloud-init did not set VULTR_REGION}"
: "${VULTR_PLAN:?cloud-init did not set VULTR_PLAN}"
: "${VULTR_DEPLOYMENT_KIND:?cloud-init did not set VULTR_DEPLOYMENT_KIND}"
: "${EXPECTED_GPU_MODEL:?cloud-init did not set EXPECTED_GPU_MODEL}"

export PATH="/root/.cargo/bin:/usr/local/cuda/bin:$PATH"
export CUDA_HOME="${CUDA_HOME:-/usr/local/cuda}"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"
export WGPU_BACKEND=vulkan

cd "$SPIKE_DIR/spikes/precision"
RESULTS_PATH="${SPIKE_RESULTS_PATH:-$PWD/RESULTS.md}"
TRANSCRIPT_PATH="$(mktemp)"
trap 'rm -f "$TRANSCRIPT_PATH"' EXIT

command -v cargo >/dev/null
command -v nvcc >/dev/null
command -v nvidia-smi >/dev/null
NVIDIA_DEVICE="$(nvidia-smi --query-gpu=name,driver_version,pci.bus_id --format=csv,noheader)"

export SEMBLA_MACHINE_KIND=nvidia
export SEMBLA_RESULTS_PATH="$RESULTS_PATH"
export SEMBLA_INFRA_GENERATED_AT_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
export SEMBLA_INFRA_REPOSITORY_COMMIT="$(git -C "$SPIKE_DIR" rev-parse HEAD)"
export SEMBLA_INFRA_PROVIDER=vultr
export SEMBLA_INFRA_VULTR_REGION="$VULTR_REGION"
export SEMBLA_INFRA_VULTR_PLAN="$VULTR_PLAN"
export SEMBLA_INFRA_VULTR_DEPLOYMENT_KIND="$VULTR_DEPLOYMENT_KIND"
export SEMBLA_INFRA_EXPECTED_GPU="$EXPECTED_GPU_MODEL"
export SEMBLA_INFRA_NVIDIA_DEVICE="$NVIDIA_DEVICE"
export SEMBLA_INFRA_REQUESTED_FP64_CLASS=full-rate
export SEMBLA_INFRA_FULL_RATE_EXTRAPOLATION=refused-until-runtime-verification

cargo build --release --features cuda
cargo run --release --features cuda 2>&1 | tee "$TRANSCRIPT_PATH"
test -s "$RESULTS_PATH"
echo "Wrote $RESULTS_PATH"
