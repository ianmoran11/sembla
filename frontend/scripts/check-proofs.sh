#!/usr/bin/env bash
set -euo pipefail

frontend_root="$(cd "$(dirname "$0")/.." && pwd)"

# Add future proof-module globs to this list, one line per module family.
proof_sources=(
  "$frontend_root"/Sembla/Lumping*.lean
)

forbidden='(^|[^[:alnum:]_])(sorry|admit|native_decide)([^[:alnum:]_]|$)|^[[:space:]]*axiom([[:space:]]|$)'
if grep -EnH "$forbidden" "${proof_sources[@]}"; then
  echo "forbidden Lean proof construct found" >&2
  exit 1
fi

required_theorems=(
  lookupTotal_bump
  lookupTotal_groupTotals
  groupedCount_eq_naiveCount
  plan_rewrite_congr
)
for theorem in "${required_theorems[@]}"; do
  if ! grep -Eq "^[[:space:]]*theorem[[:space:]]+${theorem}([[:space:](]|$)" \
      "${proof_sources[@]}"; then
    echo "required Lean theorem is missing or renamed: $theorem" >&2
    exit 1
  fi
done

# Implementation validation (2026-07): temporarily adding `sorry` to a
# Lumping module made repo-root ./scripts/check.sh fail in this guard; the
# temporary edit was reverted.
echo "Lean lumping proof hygiene checks passed"
