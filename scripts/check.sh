#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Coordinate-keyed Philox is implemented locally so its bit behavior cannot
# drift with an external RNG dependency version. Cargo resolves every valid
# dependency syntax before this check, including aliases and workspace entries.
runtime_dependencies="$(
    cargo tree \
        --package sembla-runtime \
        --all-features \
        --target all \
        --edges normal,build,dev \
        --depth 1 \
        --prefix none \
        --format '{p}' | \
        tail -n +2
)"
if [[ -n "$runtime_dependencies" ]]; then
    echo "sembla-runtime must remain dependency-free; found:" >&2
    printf '%s\n' "$runtime_dependencies" >&2
    exit 1
fi
