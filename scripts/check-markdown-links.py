#!/usr/bin/env python3
"""Check tracked Markdown files for missing relative link targets."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

LINK_START = re.compile(r"(?<!!)\[[^\]\n]*\]\(")
FENCE_LINE = re.compile(r"^\s{0,3}(?P<marker>`{3,}|~{3,})(?P<rest>.*)$")
BACKSLASH_ESCAPE = re.compile(r"\\([!\"#$%&'()*+,\-./:;<=>?@[\\\]^_`{|}~])")


@dataclass(frozen=True, order=True)
class MissingLink:
    source: str
    line: int
    target: str


def repository_root(explicit_root: Path | None) -> Path:
    if explicit_root is not None:
        root = explicit_root.resolve()
    else:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        )
        root = Path(result.stdout.strip()).resolve()
    return root


def tracked_markdown_files(root: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--", "*.md"],
        check=True,
        stdout=subprocess.PIPE,
    )
    relative_paths = [Path(os.fsdecode(item)) for item in result.stdout.split(b"\0") if item]
    return sorted(
        path
        for path in relative_paths
        if not path.parts or path.parts[0] != ".piprd"
    )


def mask_inline_code(line: str) -> str:
    """Mask simple inline-code spans without changing character positions."""
    characters = list(line)
    index = 0
    while index < len(line):
        if line[index] != "`":
            index += 1
            continue
        end_of_ticks = index
        while end_of_ticks < len(line) and line[end_of_ticks] == "`":
            end_of_ticks += 1
        ticks = line[index:end_of_ticks]
        closing = line.find(ticks, end_of_ticks)
        if closing < 0:
            index = end_of_ticks
            continue
        for masked in range(index, closing + len(ticks)):
            characters[masked] = " "
        index = closing + len(ticks)
    return "".join(characters)


def destination_at(line: str, start: int) -> str | None:
    """Read an inline-link destination beginning just after the opening parenthesis."""
    while start < len(line) and line[start].isspace():
        start += 1
    if start >= len(line):
        return None

    if line[start] == "<":
        end = start + 1
        while end < len(line):
            if line[end] == ">" and line[end - 1] != "\\":
                return line[start + 1 : end]
            end += 1
        return None

    end = start
    nested_parentheses = 0
    while end < len(line):
        character = line[end]
        if character == "\\" and end + 1 < len(line):
            end += 2
            continue
        if character == "(":
            nested_parentheses += 1
        elif character == ")":
            if nested_parentheses == 0:
                break
            nested_parentheses -= 1
        elif character.isspace() and nested_parentheses == 0:
            break
        end += 1
    return line[start:end] or None


def link_destinations(markdown: str) -> list[tuple[int, str]]:
    links: list[tuple[int, str]] = []
    fence_character: str | None = None
    fence_length = 0

    for line_number, original_line in enumerate(markdown.splitlines(), start=1):
        fence = FENCE_LINE.match(original_line)
        if fence_character is not None:
            if fence:
                marker = fence.group("marker")
                trailing_text = fence.group("rest")
                if (
                    marker[0] == fence_character
                    and len(marker) >= fence_length
                    and not trailing_text.strip()
                ):
                    fence_character = None
                    fence_length = 0
            continue

        if fence:
            marker = fence.group("marker")
            trailing_text = fence.group("rest")
            # CommonMark backtick info strings cannot themselves contain a
            # backtick. Tilde fences have no equivalent restriction.
            if marker[0] == "~" or "`" not in trailing_text:
                fence_character = marker[0]
                fence_length = len(marker)
                continue

        line = mask_inline_code(original_line)
        for match in LINK_START.finditer(line):
            destination = destination_at(original_line, match.end())
            if destination is not None:
                links.append((line_number, destination))
    return links


def local_target(root: Path, source: Path, target: str) -> Path | None:
    target = BACKSLASH_ESCAPE.sub(r"\1", target.strip())
    if not target or target.startswith("#") or target.startswith("//"):
        return None

    try:
        parsed = urlsplit(target)
    except ValueError:
        parsed = urlsplit("")
    if parsed.scheme or parsed.netloc:
        return None

    decoded_path = unquote(parsed.path)
    if not decoded_path:
        return None
    if decoded_path.startswith("/"):
        candidate = root / decoded_path.lstrip("/")
    else:
        candidate = root / source.parent / decoded_path
    return candidate.resolve(strict=False)


def check_links(root: Path) -> tuple[list[MissingLink], int, int]:
    missing: list[MissingLink] = []
    checked = 0
    files = tracked_markdown_files(root)

    for source in files:
        markdown = (root / source).read_text(encoding="utf-8")
        for line, target in link_destinations(markdown):
            candidate = local_target(root, source, target)
            if candidate is None:
                continue
            checked += 1
            try:
                candidate.relative_to(root)
                inside_repository = True
            except ValueError:
                inside_repository = False
            if not inside_repository or not candidate.exists():
                missing.append(MissingLink(source.as_posix(), line, target))

    return sorted(missing), checked, len(files)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        help="Git worktree to inspect (defaults to the current worktree)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        root = repository_root(args.root)
        missing, checked, file_count = check_links(root)
    except (OSError, subprocess.CalledProcessError, UnicodeError) as error:
        print(f"error: unable to check Markdown links: {error}", file=sys.stderr)
        return 2

    if missing:
        for issue in missing:
            print(f"{issue.source}:{issue.line} -> {issue.target}", file=sys.stderr)
        print(
            f"error: {len(missing)} missing local Markdown target(s)",
            file=sys.stderr,
        )
        return 1

    print(
        f"checked {checked} local links in {file_count} tracked Markdown files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
