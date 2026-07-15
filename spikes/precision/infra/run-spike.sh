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
REPORT_PATH="$(mktemp)"
trap 'rm -f "$TRANSCRIPT_PATH" "$REPORT_PATH"' EXIT

command -v cargo >/dev/null
command -v nvcc >/dev/null
nvidia-smi >/dev/null

# Build first, then let the PRD-0005 runner own RESULTS.md. Capturing stdout to a
# separate transcript avoids concurrently truncating the benchmark artifact.
cargo build --release --features cuda
cargo run --release --features cuda 2>&1 | tee "$TRANSCRIPT_PATH"

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

{
  echo "# Precision spike GPU run"
  echo
  echo "## PRD 0004 infrastructure metadata"
  echo
  echo "- generated-at-utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- repository-commit: $(git -C "$SPIKE_DIR" rev-parse HEAD)"
  echo "- aws-region: $AWS_REGION"
  echo "- requested-ami: $AMI_REQUEST"
  echo "- actual-ami-id: $ACTUAL_AMI_ID"
  echo "- gpu-class: $GPU_CLASS"
  echo "- fp64-class: $FP64_CLASS"
  echo "- fp64-fp32-ratio: $FP64_FP32_RATIO"
  echo "- full-rate-extrapolation: $FULL_RATE_EXTRAPOLATION"
  echo "- expected-gpu: $GPU_MODEL"
  echo "- instance-type: $INSTANCE_TYPE"
  echo
  echo "### NVIDIA device"
  echo
  echo '```text'
  nvidia-smi --query-gpu=name,driver_version,pci.bus_id --format=csv,noheader
  echo '```'
  echo

  if [ -s "$RESULTS_PATH" ]; then
    echo "## Benchmark artifact"
    echo
    cat "$RESULTS_PATH"
  else
    # Before PRD 0005 lands, the current runner prints its result rather than
    # creating RESULTS.md. Preserve that output as an explicit fallback.
    echo "## Spike command transcript"
    echo
    echo '```text'
    cat "$TRANSCRIPT_PATH"
    echo '```'
  fi
} >"$REPORT_PATH"

mv "$REPORT_PATH" "$RESULTS_PATH"
echo "Wrote $RESULTS_PATH"
