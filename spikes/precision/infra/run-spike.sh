#!/usr/bin/env bash
set -euo pipefail

# cloud-init writes the selected Terraform profile here.
# shellcheck disable=SC1091
source /etc/sembla-spike.env

: "${SPIKE_DIR:?cloud-init did not set SPIKE_DIR}"
: "${GPU_CLASS:?cloud-init did not set GPU_CLASS}"
: "${FP64_CLASS:?cloud-init did not set FP64_CLASS}"

export PATH="/home/ubuntu/.cargo/bin:/usr/local/cuda/bin:$PATH"
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

# Collect infrastructure provenance before the Rust runner atomically updates
# RESULTS.md, so the NVIDIA machine block owns all metadata and benchmark rows.
IMDS_TOKEN="$(curl --fail --silent --show-error --request PUT \
  --header 'X-aws-ec2-metadata-token-ttl-seconds: 60' \
  http://169.254.169.254/latest/api/token || true)"
ACTUAL_AMI_ID="unavailable"
if [ -n "$IMDS_TOKEN" ]; then
  ACTUAL_AMI_ID="$(curl --fail --silent --show-error \
    --header "X-aws-ec2-metadata-token: $IMDS_TOKEN" \
    http://169.254.169.254/latest/meta-data/ami-id || true)"
  ACTUAL_AMI_ID="${ACTUAL_AMI_ID:-unavailable}"
fi
NVIDIA_DEVICE="$(nvidia-smi --query-gpu=name,driver_version,pci.bus_id --format=csv,noheader)"

export SEMBLA_MACHINE_KIND=nvidia
export SEMBLA_RESULTS_PATH="$RESULTS_PATH"
export SEMBLA_INFRA_GENERATED_AT_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
export SEMBLA_INFRA_REPOSITORY_COMMIT="$(git -C "$SPIKE_DIR" rev-parse HEAD)"
export SEMBLA_INFRA_AWS_REGION="$AWS_REGION"
export SEMBLA_INFRA_REQUESTED_AMI="$AMI_REQUEST"
export SEMBLA_INFRA_ACTUAL_AMI_ID="$ACTUAL_AMI_ID"
export SEMBLA_INFRA_GPU_CLASS="$GPU_CLASS"
export SEMBLA_INFRA_FP64_CLASS="$FP64_CLASS"
export SEMBLA_INFRA_FP64_FP32_RATIO="$FP64_FP32_RATIO"
export SEMBLA_INFRA_FULL_RATE_EXTRAPOLATION="$FULL_RATE_EXTRAPOLATION"
export SEMBLA_INFRA_EXPECTED_GPU="$GPU_MODEL"
export SEMBLA_INFRA_NVIDIA_DEVICE="$NVIDIA_DEVICE"
export SEMBLA_INFRA_INSTANCE_TYPE="$INSTANCE_TYPE"

# RESULTS.md must already contain the Mac state if this run is intended to
# assemble both machines. The Rust process reads, merges, and atomically replaces
# that artifact; tee captures diagnostics without wrapping or truncating it.
cargo build --release --features cuda
cargo run --release --features cuda 2>&1 | tee "$TRANSCRIPT_PATH"
test -s "$RESULTS_PATH"
echo "Wrote $RESULTS_PATH"
