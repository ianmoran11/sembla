#!/usr/bin/env python3
"""Cross-check a PRD's allowed-file list against the paths its body names.

Five managed runs have now stalled on the same defect: a PRD requires work on a
file its own allowed-file list omits, so every attempt is correctly rejected as
a scope violation and no in-run action can clear it. `DECISIONS.md` §I7 already
says allowed-file lists must be derived from the acceptance criteria, and §M2
says the runner should stop and report. Neither is a check, and the omission is
invisible to the author precisely because they were thinking about the file when
they wrote the criterion.

This is advisory, not a gate. It cannot tell a file the PRD will *edit* from one
it merely *cites* as evidence, and PRDs legitimately cite a great deal. What it
does is print a short list to eyeball, which is enough: the failures so far were
all a path named in an imperative sentence and absent from the list.

    python3 scripts/check-prd-allowlist.py docs/prds-foo/0001-bar.md
    python3 scripts/check-prd-allowlist.py --all

Exits non-zero only on a usage or parse error, never on findings.
"""

from __future__ import annotations

import argparse
import fnmatch
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# A path-ish token inside backticks: at least one "/" or a known source suffix,
# no spaces. Trailing ":123" line references and punctuation are trimmed later.
TOKEN = re.compile(r"`([^`\n]+)`")
SUFFIXES = (
    ".rs", ".md", ".sh", ".py", ".json", ".toml", ".lean", ".yml", ".yaml",
)
# Sections that describe scope rather than demand work.
CONTEXT_HEADINGS = ("non-goals", "deliberately excluded", "context")

# Gates every PRD is required to run and none is expected to edit. Naming them
# is not a scope signal, and flagging them buries the real findings.
ALWAYS_RUN_NEVER_EDIT = {
    "scripts/check-rust.sh",
    "scripts/check-markdown-links.py",
    "scripts/check-prd-allowlist.py",
}


def tracked_files() -> list[str]:
    import subprocess

    result = subprocess.run(
        ["git", "-C", str(REPO), "ls-files"],
        capture_output=True, text=True, check=True,
    )
    return result.stdout.splitlines()


def basename_index(files: list[str]) -> dict[str, list[str]]:
    index: dict[str, list[str]] = {}
    for path in files:
        index.setdefault(path.rsplit("/", 1)[-1], []).append(path)
    return index


def extract_allowlist(body: str) -> list[str]:
    """Return the glob patterns under the '## Allowed files' heading."""
    match = re.search(
        r"^##+\s*Allowed files\s*$(.*?)(?=^##\s|\Z)", body, re.M | re.S
    )
    if not match:
        return []
    patterns: list[str] = []
    for line in match.group(1).splitlines():
        stripped = line.strip()
        if not stripped.startswith(("-", "*")):
            continue
        # A bullet may carry several comma-separated paths plus prose.
        for token in TOKEN.findall(stripped):
            patterns.append(normalise(token))
    return [p for p in patterns if p]


def normalise(token: str) -> str:
    token = token.strip().strip(",.;:")
    # Drop `file.rs:1828` and `file.rs:1828-1833` line references.
    token = re.sub(r":\d+(-\d+)?$", "", token)
    return token


def looks_like_path(token: str) -> bool:
    if not token or " " in token or token.startswith(("http", "$", "-")):
        return False
    return "/" in token or token.endswith(SUFFIXES)


def covered(path: str, patterns: list[str]) -> bool:
    for pattern in patterns:
        if fnmatch.fnmatch(path, pattern):
            return True
        # A directory-ish pattern covers everything beneath it.
        if pattern.endswith("/**") and path.startswith(pattern[:-3]):
            return True
        if pattern.endswith("**") and path.startswith(pattern[:-2]):
            return True
        if path == pattern or path.startswith(pattern.rstrip("/") + "/"):
            return True
    return False


def section_of(body: str, index: int) -> str:
    """The nearest preceding heading, lowercased."""
    headings = [
        (m.start(), m.group(1).strip().lower())
        for m in re.finditer(r"^##+\s*(.+?)\s*$", body, re.M)
    ]
    current = ""
    for start, title in headings:
        if start > index:
            break
        current = title
    return current


def check(prd: pathlib.Path, index: dict[str, list[str]]) -> int:
    body = prd.read_text()
    patterns = extract_allowlist(body)
    if not patterns:
        print(f"{prd.relative_to(REPO)}: no '## Allowed files' section found")
        return 0

    allowed_span = re.search(
        r"^##+\s*Allowed files\s*$(.*?)(?=^##\s|\Z)", body, re.M | re.S
    )
    findings: dict[str, str] = {}
    for match in TOKEN.finditer(body):
        start = match.start()
        if allowed_span and allowed_span.start() <= start < allowed_span.end():
            continue
        token = normalise(match.group(1))
        if not looks_like_path(token):
            continue
        # PRDs often name a file by basename alone -- "a stage in
        # `run-demographic-benchmark.sh`". That is exactly how the omission that
        # stalled prds-sweep-throughput/0001 escaped an earlier version of this
        # check, so resolve bare names against the tracked tree. Ambiguous
        # basenames are skipped rather than guessed.
        if "/" not in token:
            candidates = index.get(token, [])
            if len(candidates) != 1:
                continue
            token = candidates[0]
        elif not (REPO / token).is_file():
            # Directories and bare "/" are prose, not scope signals.
            continue
        if token in ALWAYS_RUN_NEVER_EDIT:
            continue
        if covered(token, patterns):
            continue
        heading = section_of(body, start)
        if any(h in heading for h in CONTEXT_HEADINGS):
            continue
        findings.setdefault(token, heading or "(preamble)")

    try:
        name = prd.relative_to(REPO)
    except ValueError:  # a file outside the repo, e.g. a `git show` snapshot
        name = prd
    if not findings:
        print(f"{name}: OK — every path named outside context sections is allowed")
        return 0

    print(f"{name}: {len(findings)} path(s) named but not in Allowed files")
    for path, heading in sorted(findings.items()):
        print(f"    {path}   (under: {heading})")
    print("    Confirm each is read-only context. If the PRD requires changing")
    print("    one, add it to Allowed files now — see DECISIONS.md §I7 and §M2.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prd", nargs="*", help="PRD markdown files")
    parser.add_argument("--all", action="store_true", help="check every PRD")
    args = parser.parse_args()

    if args.all:
        targets = sorted(REPO.glob("docs/prds-*/[0-9]*.md"))
    elif args.prd:
        targets = [pathlib.Path(p).resolve() for p in args.prd]
    else:
        parser.error("pass PRD paths or --all")

    index = basename_index(tracked_files())
    for target in targets:
        if not target.exists():
            print(f"error: {target} does not exist", file=sys.stderr)
            return 2
        check(target, index)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
