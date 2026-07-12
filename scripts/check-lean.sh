#!/usr/bin/env bash
set -euo pipefail
if command -v lake >/dev/null 2>&1; then
    bash frontend/scripts/check-parity.sh
else
    echo "warning: lake not found; skipping Lean frontend build and parity checks" >&2
fi
