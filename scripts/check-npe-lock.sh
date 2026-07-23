#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

image="python:3.12.8-slim-bookworm@sha256:8859bd6ca943079262c27e38b7119cdacede77c463139a15651dd340087a6cc9"

python3 calibration/npe/tests/test_requirements_ci_lock.py

if ! command -v docker >/dev/null 2>&1; then
  echo "error: Docker is required for the authoritative NPE lock validation" >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  echo "error: Docker is installed but its daemon is unavailable" >&2
  exit 1
fi

docker run --rm \
  --platform linux/amd64 \
  --mount "type=bind,src=$root,dst=/repo,readonly" \
  --env PIP_DISABLE_PIP_VERSION_CHECK=1 \
  --env PYTHONDONTWRITEBYTECODE=1 \
  --env SEMBLA_NPE_LOCK_CONTAINER=1 \
  --workdir /repo \
  "$image" \
  bash -ceu '
    tmp="$(mktemp -d)"
    trap '\''rm -rf "$tmp"'\'' EXIT

    python calibration/npe/tests/test_requirements_ci_lock.py
    calibration/npe/generate-requirements-ci-lock.sh "$tmp/requirements-ci.lock"
    if ! cmp -s calibration/npe/requirements-ci.lock "$tmp/requirements-ci.lock"; then
      echo "error: requirements-ci.lock differs from deterministic regeneration" >&2
      diff -u calibration/npe/requirements-ci.lock "$tmp/requirements-ci.lock" || true
      exit 1
    fi
    echo "NPE CI lock regenerates byte-identically"

    python -m venv "$tmp/runtime"
    PYTHON="$tmp/runtime/bin/python" \
      calibration/npe/install-requirements-ci.sh 2>&1 | tee "$tmp/install.log"
    if grep -q "Installing build dependencies" "$tmp/install.log"; then
      echo "error: pip resolved an isolated, unhashed build environment" >&2
      exit 1
    fi

    PYTHON="$tmp/runtime/bin/python" ./scripts/check-npe-smoke.sh
  '
