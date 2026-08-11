#!/usr/bin/env python3

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check-architecture-canvases.py"
NESTED = ("Contracts", "Estimation", "Frontend", "Runtime")


class ArchitectureCanvasCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.directory = self.root / "docs" / "architecture"
        self.directory.mkdir(parents=True)
        for name in NESTED:
            self.write_canvas(name, [], [])
        detail_links = [
            {
                "id": f"{name.lower()}_link",
                "type": "text",
                "text": f"[Open {name}]({name}.canvas)",
                "x": index * 200,
                "y": 0,
                "width": 180,
                "height": 120,
            }
            for index, name in enumerate(NESTED)
        ]
        self.write_canvas("Architecture", detail_links, [])

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_canvas(
        self, name: str, nodes: list[dict[str, object]], edges: list[dict[str, object]]
    ) -> None:
        (self.directory / f"{name}.canvas").write_text(
            json.dumps(
                {
                    "metadata": {"version": "1.0-1.0", "frontmatter": {}},
                    "nodes": nodes,
                    "edges": edges,
                }
            ),
            encoding="utf-8",
        )

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_valid_linked_hierarchy_passes(self) -> None:
        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("5 canvases, 4 nodes, 0 edges", result.stdout)

    def test_missing_detail_target_fails(self) -> None:
        (self.directory / "Runtime.canvas").unlink()

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("target does not exist", result.stderr)
        self.assertIn("architecture canvas set must be", result.stderr)

    def test_live_portal_in_overview_fails(self) -> None:
        payload = json.loads((self.directory / "Architecture.canvas").read_text())
        payload["nodes"].append(
            {
                "id": "frontend_portal",
                "type": "file",
                "file": "docs/architecture/Frontend.canvas",
                "portal": True,
                "x": 0,
                "y": 200,
                "width": 180,
                "height": 120,
            }
        )
        self.write_canvas("Architecture", payload["nodes"], [])

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("instead of embedding live portals", result.stderr)

    def test_file_preview_node_fails(self) -> None:
        node = {
            "id": "preview",
            "type": "file",
            "file": "docs/architecture/Frontend.canvas",
            "x": 0,
            "y": 0,
            "width": 180,
            "height": 120,
        }
        self.write_canvas("Contracts", [node], [])

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must use a compact text card", result.stderr)

    def test_overlapping_content_nodes_fail(self) -> None:
        nodes = [
            {
                "id": "first",
                "type": "text",
                "text": "first",
                "x": 0,
                "y": 0,
                "width": 100,
                "height": 100,
            },
            {
                "id": "second",
                "type": "text",
                "text": "second",
                "x": 50,
                "y": 50,
                "width": 100,
                "height": 100,
            },
        ]
        self.write_canvas("Contracts", nodes, [])

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("content nodes overlap: first and second", result.stderr)

    def test_dangling_unstyled_edge_fails(self) -> None:
        node = {
            "id": "source",
            "type": "text",
            "text": "source",
            "x": 0,
            "y": 0,
            "width": 100,
            "height": 100,
        }
        edge = {
            "id": "bad_edge",
            "fromNode": "source",
            "fromSide": "right",
            "toNode": "missing",
            "toSide": "left",
        }
        self.write_canvas("Contracts", [node], [edge])

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("has a missing endpoint", result.stderr)
        self.assertIn("requires Advanced Canvas styleAttributes", result.stderr)


if __name__ == "__main__":
    unittest.main()
