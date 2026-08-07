#!/usr/bin/env bash
# Recompute and verify every hash and semantic coordinate in an Australian
# population chain report.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
exec python3 data/abs/chain.py verify "$@"
