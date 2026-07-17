#!/usr/bin/env bash
set -Eeuo pipefail

# Written by cloud-init; contains no credentials.
# shellcheck disable=SC1091
source /etc/sembla-spike.env

: "${SPIKE_DIR:?cloud-init did not set SPIKE_DIR}"
: "${HYPERSTACK_REGION:?cloud-init did not set HYPERSTACK_REGION}"
: "${HYPERSTACK_ENVIRONMENT:?cloud-init did not set HYPERSTACK_ENVIRONMENT}"
: "${HYPERSTACK_FLAVOR:?cloud-init did not set HYPERSTACK_FLAVOR}"
: "${HYPERSTACK_IMAGE:?cloud-init did not set HYPERSTACK_IMAGE}"
: "${EXPECTED_GPU_MODEL:?cloud-init did not set EXPECTED_GPU_MODEL}"
: "${EXPECTED_GPU_COUNT:?cloud-init did not set EXPECTED_GPU_COUNT}"
: "${CUDA_HOME:?cloud-init did not set CUDA_HOME}"
: "${SPIKE_RUN_ID:?set SPIKE_RUN_ID to the collector-generated unique run identity}"
if [[ ! "$SPIKE_RUN_ID" =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]]; then
  echo "SPIKE_RUN_ID must contain only lowercase alphanumeric hyphen-separated tokens" >&2
  exit 2
fi

export PATH="$HOME/.cargo/bin:$CUDA_HOME/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"
export WGPU_BACKEND=vulkan

cd "$SPIKE_DIR/spikes/precision"
RESULTS_PATH="${SPIKE_RESULTS_PATH:?set SPIKE_RESULTS_PATH to a distinct absolute seeded result file}"
case "$RESULTS_PATH" in
  /*) ;;
  *) echo "SPIKE_RESULTS_PATH must be absolute" >&2; exit 2 ;;
esac

test -f "$RESULTS_PATH"
command -v nvidia-smi >/dev/null
command -v nvcc >/dev/null

NVIDIA_DEVICE="$(nvidia-smi --query-gpu=name,driver_version,pci.bus_id --format=csv,noheader)"
ACTUAL_GPU_COUNT="$(printf '%s\n' "$NVIDIA_DEVICE" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"
if [[ "$ACTUAL_GPU_COUNT" != "$EXPECTED_GPU_COUNT" ]]; then
  echo "expected $EXPECTED_GPU_COUNT GPU, but nvidia-smi reported $ACTUAL_GPU_COUNT" >&2
  exit 1
fi
if ! printf '%s\n' "$NVIDIA_DEVICE" | grep -Eqi -- "(^|[^[:alnum:]])${EXPECTED_GPU_MODEL}([^[:alnum:]]|$)"; then
  echo "nvidia-smi device does not contain the exact model token $EXPECTED_GPU_MODEL: $NVIDIA_DEVICE" >&2
  exit 1
fi

REPOSITORY_COMMIT="$(git -C "$SPIKE_DIR" rev-parse HEAD)"
RUN_STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'SEMBLA_RUN_START run_id=%s started_at=%s repository_commit=%s\n' \
  "$SPIKE_RUN_ID" "$RUN_STARTED_AT" "$REPOSITORY_COMMIT"

export SEMBLA_MACHINE_KIND=nvidia
export SEMBLA_RESULTS_PATH="$RESULTS_PATH"
export SEMBLA_INFRA_GENERATED_AT_UTC="$RUN_STARTED_AT"
export SEMBLA_INFRA_REPOSITORY_COMMIT="$REPOSITORY_COMMIT"
export SEMBLA_INFRA_RUN_ID="$SPIKE_RUN_ID"
export SEMBLA_INFRA_PROVIDER=hyperstack
export SEMBLA_INFRA_HYPERSTACK_REGION="$HYPERSTACK_REGION"
export SEMBLA_INFRA_HYPERSTACK_ENVIRONMENT="$HYPERSTACK_ENVIRONMENT"
export SEMBLA_INFRA_HYPERSTACK_FLAVOR="$HYPERSTACK_FLAVOR"
export SEMBLA_INFRA_HYPERSTACK_IMAGE="$HYPERSTACK_IMAGE"
export SEMBLA_INFRA_EXPECTED_GPU="$EXPECTED_GPU_MODEL"
export SEMBLA_INFRA_NVIDIA_DEVICE="$NVIDIA_DEVICE"
export SEMBLA_INFRA_REQUESTED_FP64_CLASS=full-rate
export SEMBLA_INFRA_FULL_RATE_EXTRAPOLATION=refused-until-runtime-verification

TRANSCRIPT_PATH="$(mktemp /tmp/sembla-precision.XXXXXX.log)"
trap 'rm -f "$TRANSCRIPT_PATH"' EXIT
cargo run --locked --release --features cuda 2>&1 | tee "$TRANSCRIPT_PATH"

test -s "$RESULTS_PATH"
RESULT_SHA256="$(sha256sum "$RESULTS_PATH" | awk '{print $1}')"
printf 'SEMBLA_RUN_COMPLETE run_id=%s result_sha256=%s repository_commit=%s\n' \
  "$SPIKE_RUN_ID" "$RESULT_SHA256" "$REPOSITORY_COMMIT"
echo "Wrote $RESULTS_PATH"
