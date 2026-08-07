"""Acquire the pinned ABS releases into the local cache.

This is the only module in the pipeline permitted to touch the network, and it
does so only when explicitly asked (DECISIONS.md N10). Without ``--download`` it
verifies the cache and reports, so no build can acquire a network side effect by
accident.

`sources.json` is never rewritten here. ``--refresh`` prints the newly observed
hash for a human to paste in, so an upstream revision surfaces as a reviewed
change rather than being silently absorbed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
SOURCES = HERE / "sources.json"
CACHE = HERE / "cache"

_USER_AGENT = "sembla-abs-pipeline/1.0 (+https://github.com/ianmoran11/sembla)"


def load_sources() -> dict:
    return json.loads(SOURCES.read_text(encoding="utf-8"))["sources"]


def cache_path(entry: dict) -> pathlib.Path:
    """Return a safe direct child of CACHE, rejecting traversal and symlinks."""
    filename = entry.get("filename")
    if (
        not isinstance(filename, str)
        or not filename
        or pathlib.PurePath(filename).name != filename
        or "/" in filename
        or "\\" in filename
    ):
        raise ValueError(f"unsafe cache filename {filename!r}")
    root = CACHE.resolve()
    path = CACHE / filename
    resolved = path.resolve(strict=False)
    if resolved.parent != root or path.is_symlink():
        raise ValueError(f"cache path escapes or is a symlink: {path}")
    return path


def digest(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def verify(sources: dict) -> dict[str, str]:
    """Return ``{id: status}`` where status is ok, missing or hash-mismatch."""
    status = {}
    for key, entry in sorted(sources.items()):
        path = cache_path(entry)
        if not path.exists():
            status[key] = "missing"
        elif digest(path) != entry["sha256"]:
            status[key] = "hash-mismatch"
        else:
            status[key] = "ok"
    return status


def download(url: str, destination: pathlib.Path) -> None:
    destination = pathlib.Path(destination)
    root = CACHE.resolve()
    if destination.resolve(strict=False).parent != root or destination.is_symlink():
        raise ValueError(f"download destination is outside cache: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(url, headers={"User-Agent": _USER_AGENT})
    with urllib.request.urlopen(request, timeout=120) as response:
        payload = response.read()
    destination.write_bytes(payload)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--download",
        action="store_true",
        help="fetch any missing or mismatched source into the cache",
    )
    parser.add_argument(
        "--refresh",
        metavar="ID",
        help="re-download one source and print its hash without editing sources.json",
    )
    args = parser.parse_args(argv)
    sources = load_sources()

    if args.refresh:
        if args.refresh not in sources:
            print(f"unknown source id {args.refresh!r}", file=sys.stderr)
            return 2
        entry = sources[args.refresh]
        path = cache_path(entry)
        download(entry["url"], path)
        observed = digest(path)
        print(f"{args.refresh}: {observed} ({path.stat().st_size} bytes)")
        if observed != entry["sha256"]:
            print(
                "  differs from sources.json; paste the new hash and byte count "
                "by hand after reviewing the upstream change",
                file=sys.stderr,
            )
            return 1
        print("  unchanged from sources.json")
        return 0

    status = verify(sources)
    if args.download:
        for key, state in sorted(status.items()):
            if state == "ok":
                continue
            entry = sources[key]
            download(entry["url"], cache_path(entry))
        status = verify(sources)

    failures = 0
    for key, state in sorted(status.items()):
        print(f"{key:14s} {state}")
        if state != "ok":
            failures += 1
    if failures:
        hint = "" if args.download else "; re-run with --download to acquire"
        print(f"\n{failures} source(s) not ok{hint}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
