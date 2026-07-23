#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: Rust-only checks require cargo; install the toolchain pinned by rust-toolchain.toml" >&2
    exit 1
fi
if ! command -v git >/dev/null 2>&1; then
    echo "error: Rust-only checks require git to verify that Cargo.lock is unchanged" >&2
    exit 1
fi

cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace

if ! grep -Eq '^libm[[:space:]]*=[[:space:]]*"=[0-9]+\.[0-9]+\.[0-9]+"[[:space:]]*$' \
    crates/sembla-runtime/Cargo.toml; then
    echo 'sembla-runtime must pin libm with an exact =x.y.z requirement' >&2
    exit 1
fi

# Philox remains local: Cargo-resolved dependency identities prevent aliases,
# target tables, optional features, or workspace inheritance from bypassing
# the approved direct-dependency policy.
unexpected_runtime_dependencies="$(
    cargo tree --locked \
        --package sembla-runtime \
        --all-features \
        --target all \
        --edges normal,build,dev \
        --depth 1 \
        --prefix none \
        --format '{p}' | \
        tail -n +2 | \
        awk '$1 != "sembla-ir" && $1 != "sha2" && $1 != "libm"'
)"
if [[ -n "$unexpected_runtime_dependencies" ]]; then
    echo "unapproved dependencies are forbidden in sembla-runtime; found:" >&2
    printf '%s\n' "$unexpected_runtime_dependencies" >&2
    exit 1
fi

rng_dependencies="$(
    cargo tree --locked \
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

if ! git diff --exit-code HEAD -- Cargo.lock; then
    echo "error: Rust validation changed Cargo.lock; restore it and update dependencies explicitly" >&2
    exit 1
fi

echo "Rust formatting, lint, tests, dependency policy, and lock checks passed"
