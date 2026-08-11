#!/usr/bin/env python3
"""Validate the Advanced JSON Canvas architecture atlas and its linked details."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[1]
CANVAS_DIR = Path("docs/architecture")
REQUIRED_CANVASES = {
    "Architecture.canvas",
    "Contracts.canvas",
    "Estimation.canvas",
    "Frontend.canvas",
    "Runtime.canvas",
}
REQUIRED_SUBCANVAS_LINKS = {
    "docs/architecture/Contracts.canvas",
    "docs/architecture/Estimation.canvas",
    "docs/architecture/Frontend.canvas",
    "docs/architecture/Runtime.canvas",
}
MARKDOWN_LINK_PATTERN = re.compile(r"(?<!!)\[[^]]+\]\(([^)]+)\)")
URL_SCHEME_PATTERN = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
NODE_TYPES = {"text", "file", "link", "group"}
SIDES = {"top", "right", "bottom", "left"}
SHAPES = {
    "rectangle",
    "pill",
    "diamond",
    "parallelogram",
    "circle",
    "predefined-process",
    "document",
    "database",
}
PATH_STYLES = {"solid", "long-dashed", "short-dashed", "dotted"}
ARROW_STYLES = {
    "triangle",
    "triangle-outline",
    "thin-triangle",
    "halved-triangle",
    "diamond",
    "diamond-outline",
    "circle",
    "circle-outline",
    "blunt",
}
PATHFINDING_METHODS = {"bezier", "direct", "square", "a-star"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    return parser.parse_args()


def validate_canvas(
    root: Path, path: Path
) -> tuple[list[str], int, int, set[str], set[str]]:
    errors: list[str] = []
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path.name}: root must be an object")
    metadata = payload.get("metadata")
    if not isinstance(metadata, dict) or metadata.get("version") != "1.0-1.0":
        errors.append(f"{path.name}: metadata.version must be Advanced JSON Canvas 1.0-1.0")
    nodes = payload.get("nodes")
    edges = payload.get("edges")
    if not isinstance(nodes, list) or not isinstance(edges, list):
        raise ValueError(f"{path.name}: nodes and edges must be arrays")

    node_ids: set[str] = set()
    portals: set[str] = set()
    local_links: set[str] = set()
    content_boxes: list[tuple[str, int, int, int, int]] = []
    for index, node in enumerate(nodes):
        if not isinstance(node, dict):
            errors.append(f"{path.name}: node {index} must be an object")
            continue
        identifier = node.get("id")
        if not isinstance(identifier, str) or not identifier:
            errors.append(f"{path.name}: node {index} has no string id")
            continue
        if identifier in node_ids:
            errors.append(f"{path.name}: duplicate node id {identifier}")
        node_ids.add(identifier)
        if "-" in identifier:
            errors.append(f"{path.name}: node id should not contain '-': {identifier}")
        if node.get("type") not in NODE_TYPES:
            errors.append(f"{path.name}: node {identifier} has unsupported type {node.get('type')!r}")
        for field in ("x", "y", "width", "height"):
            if not isinstance(node.get(field), int):
                errors.append(f"{path.name}: node {identifier} field {field} must be integer")
        if isinstance(node.get("width"), int) and node["width"] <= 0:
            errors.append(f"{path.name}: node {identifier} width must be positive")
        if isinstance(node.get("height"), int) and node["height"] <= 0:
            errors.append(f"{path.name}: node {identifier} height must be positive")
        if node.get("type") != "group" and all(
            isinstance(node.get(field), int) for field in ("x", "y", "width", "height")
        ):
            content_boxes.append(
                (identifier, node["x"], node["y"], node["width"], node["height"])
            )
        style = node.get("styleAttributes", {})
        if not isinstance(style, dict):
            errors.append(f"{path.name}: node {identifier} styleAttributes must be an object")
        elif style.get("shape") is not None and style["shape"] not in SHAPES:
            errors.append(f"{path.name}: node {identifier} has unsupported shape {style['shape']!r}")
        if node.get("type") == "file":
            errors.append(
                f"{path.name}: file preview node {identifier} must use a compact text card"
            )
            file_name = node.get("file")
            if not isinstance(file_name, str) or not file_name:
                errors.append(f"{path.name}: file node {identifier} has no file")
            elif not (root / file_name).exists():
                errors.append(f"{path.name}: file node {identifier} target does not exist: {file_name}")
            if node.get("portal") is True:
                portals.add(str(file_name))
                if not str(file_name).endswith(".canvas"):
                    errors.append(f"{path.name}: portal {identifier} must target a canvas")
        if node.get("type") == "text" and isinstance(node.get("text"), str):
            for match in MARKDOWN_LINK_PATTERN.finditer(node["text"]):
                target = match.group(1).split("#", 1)[0].strip().strip("<>")
                if not target or URL_SCHEME_PATTERN.match(target):
                    continue
                resolved = (path.parent / target).resolve()
                try:
                    relative = resolved.relative_to(root).as_posix()
                except ValueError:
                    errors.append(
                        f"{path.name}: text node {identifier} link escapes repository: {target}"
                    )
                    continue
                local_links.add(relative)
                if not resolved.exists():
                    errors.append(
                        f"{path.name}: text node {identifier} target does not exist: {target}"
                    )

    for index, first in enumerate(content_boxes):
        first_id, first_x, first_y, first_width, first_height = first
        for second in content_boxes[index + 1 :]:
            second_id, second_x, second_y, second_width, second_height = second
            horizontal_overlap = min(
                first_x + first_width, second_x + second_width
            ) - max(first_x, second_x)
            vertical_overlap = min(
                first_y + first_height, second_y + second_height
            ) - max(first_y, second_y)
            if horizontal_overlap > 0 and vertical_overlap > 0:
                errors.append(
                    f"{path.name}: content nodes overlap: {first_id} and {second_id}"
                )

    edge_ids: set[str] = set()
    for index, edge in enumerate(edges):
        if not isinstance(edge, dict):
            errors.append(f"{path.name}: edge {index} must be an object")
            continue
        identifier = edge.get("id")
        if not isinstance(identifier, str) or not identifier:
            errors.append(f"{path.name}: edge {index} has no string id")
            continue
        if identifier in edge_ids:
            errors.append(f"{path.name}: duplicate edge id {identifier}")
        edge_ids.add(identifier)
        if "-" in identifier:
            errors.append(f"{path.name}: edge id should not contain '-': {identifier}")
        if edge.get("fromNode") not in node_ids or edge.get("toNode") not in node_ids:
            errors.append(f"{path.name}: edge {identifier} has a missing endpoint")
        for field in ("fromSide", "toSide"):
            if edge.get(field) not in SIDES:
                errors.append(f"{path.name}: edge {identifier} has invalid {field}")
        style = edge.get("styleAttributes")
        if not isinstance(style, dict):
            errors.append(f"{path.name}: edge {identifier} requires Advanced Canvas styleAttributes")
            continue
        if style.get("path") not in PATH_STYLES:
            errors.append(f"{path.name}: edge {identifier} has unsupported path style")
        if style.get("arrow") not in ARROW_STYLES:
            errors.append(f"{path.name}: edge {identifier} has unsupported arrow style")
        if style.get("pathfindingMethod") not in PATHFINDING_METHODS:
            errors.append(f"{path.name}: edge {identifier} has unsupported pathfinding method")
    return errors, len(nodes), len(edges), portals, local_links


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    canvas_directory = root / CANVAS_DIR
    try:
        paths = sorted(canvas_directory.glob("*.canvas"))
        names = {path.name for path in paths}
        errors = []
        if names != REQUIRED_CANVASES:
            errors.append(
                "architecture canvas set must be "
                f"{', '.join(sorted(REQUIRED_CANVASES))}; found {', '.join(sorted(names))}"
            )
        node_count = 0
        edge_count = 0
        top_level_portals: set[str] = set()
        top_level_links: set[str] = set()
        for path in paths:
            canvas_errors, nodes, edges, portals, local_links = validate_canvas(root, path)
            errors.extend(canvas_errors)
            node_count += nodes
            edge_count += edges
            if path.name == "Architecture.canvas":
                top_level_portals = portals
                top_level_links = {
                    link for link in local_links if link.endswith(".canvas")
                }
        if top_level_portals:
            errors.append(
                "Architecture.canvas must link to detail canvases instead of embedding "
                f"live portals; found {', '.join(sorted(top_level_portals))}"
            )
        if top_level_links != REQUIRED_SUBCANVAS_LINKS:
            errors.append(
                "Architecture.canvas detail links must be "
                f"{', '.join(sorted(REQUIRED_SUBCANVAS_LINKS))}; found "
                f"{', '.join(sorted(top_level_links))}"
            )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if errors:
        for error in sorted(errors):
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"Advanced Canvas architecture passed: {len(paths)} canvases, "
        f"{node_count} nodes, {edge_count} edges"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
