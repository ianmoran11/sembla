#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Philox remains local: Cargo-resolved dependency identities prevent aliases,
# target tables, optional features, or workspace inheritance from bypassing
# the approved direct-dependency policy.
unexpected_runtime_dependencies="$(
    cargo tree \
        --package sembla-runtime \
        --all-features \
        --target all \
        --edges normal,build,dev \
        --depth 1 \
        --prefix none \
        --format '{p}' | \
        tail -n +2 | \
        awk '$1 != "sembla-ir" && $1 != "sha2"'
)"
if [[ -n "$unexpected_runtime_dependencies" ]]; then
    echo "unapproved dependencies are forbidden in sembla-runtime; found:" >&2
    printf '%s\n' "$unexpected_runtime_dependencies" >&2
    exit 1
fi

rng_dependencies="$(
    cargo tree \
        --package sembla-runtime \
        --all-features \
        --target all \
        --edges normal,build,dev \
        --prefix none \
        --format '{p}' | \
        awk '{ print $1 }' | \
        grep -E '^(rand|rand_[[:alnum:]_-]*|getrandom|fastrand|oorandom|random123)$' || true
)"
if [[ -n "$rng_dependencies" ]]; then
    echo "external RNG dependencies are forbidden in sembla-runtime; found:" >&2
    printf '%s\n' "$rng_dependencies" >&2
    exit 1
fi
