#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

require_tool() {
    local tool="$1" guidance="$2"
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: complete repository check requires '$tool'; $guidance" >&2
        exit 1
    fi
}

require_tool cargo "install the Rust toolchain pinned by rust-toolchain.toml"
require_tool git "install git so Cargo.lock and tracked documentation can be verified"
require_tool python3 "install Python 3; the documentation checks use only its standard library"
require_tool lake "install elan so frontend/lean-toolchain provides Lake; for Rust-only validation run ./scripts/check-rust.sh"

python3 -B -m unittest discover -s scripts/tests -p 'test_*.py'
python3 -B scripts/check-markdown-links.py
python3 -B scripts/check-cargo-metadata.py
./scripts/check-rust.sh
bash frontend/scripts/check-proofs.sh
bash frontend/scripts/check-parity.sh

if ! git diff --exit-code HEAD -- Cargo.lock; then
    echo "error: complete repository validation changed Cargo.lock; restore it and update dependencies explicitly" >&2
    exit 1
fi

echo "complete documentation, Rust, Lean proof-hygiene, parity, and lock checks passed"
