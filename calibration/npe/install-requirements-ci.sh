#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

python_bin="${PYTHON:-python}"
lock="calibration/npe/requirements-ci.lock"

"$python_bin" calibration/npe/tests/test_requirements_ci_lock.py

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# nflows 0.14 is available only as an sdist. Extract its exact, hashed build
# prerequisites from the complete lock, install them without dependencies, and
# then disable build isolation so pip cannot resolve an untracked build env.
"$python_bin" - "$lock" "$tmp/build-prerequisites.txt" <<'PY'
from pathlib import Path
import re
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines(keepends=True)
out = Path(sys.argv[2])
required = {"setuptools", "wheel"}
blocks: dict[str, list[str]] = {}
current_name = None
current: list[str] = []

for line in source:
    match = re.match(r"^([A-Za-z0-9][A-Za-z0-9._-]*)==", line)
    if match:
        if current_name is not None:
            blocks[current_name] = current
        current_name = match.group(1).lower().replace("_", "-")
        current = [line]
    elif current_name is not None and (line.startswith((" ", "\t")) or not line.strip()):
        current.append(line)
    elif current_name is not None:
        blocks[current_name] = current
        current_name = None
        current = []
if current_name is not None:
    blocks[current_name] = current

missing = sorted(required - blocks.keys())
if missing:
    raise SystemExit(f"error: lock missing build prerequisites: {', '.join(missing)}")

with out.open("w", encoding="utf-8") as handle:
    handle.write("--index-url https://pypi.org/simple\n")
    for name in sorted(required):
        handle.writelines(blocks[name])
PY

"$python_bin" -m pip install \
  --disable-pip-version-check \
  --no-deps \
  --require-hashes \
  -r "$tmp/build-prerequisites.txt"

"$python_bin" -m pip install \
  --disable-pip-version-check \
  --no-build-isolation \
  --require-hashes \
  -r "$lock"
