#!/usr/bin/env bash
# Execute a deterministic annual Australian population chain through the
# standard-library-only driver. This is composition of existing public CLI
# commands, not a checkpoint or run-management subsystem.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
exec python3 data/abs/chain.py run "$@"
