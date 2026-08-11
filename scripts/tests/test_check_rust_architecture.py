#!/usr/bin/env python3

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check-rust-architecture.py"
PACKAGE_NAMES = ("sembla-ir", "sembla-runtime", "sembla-cuda", "sembla-cli")
EXPECTED_EDGES = {
    "sembla-ir": [],
    "sembla-runtime": ["sembla-ir"],
    "sembla-cuda": ["sembla-ir", "sembla-runtime"],
    "sembla-cli": ["sembla-cuda", "sembla-ir", "sembla-runtime"],
}


class RustArchitectureCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for crate in ("sembla-ir", "sembla-runtime"):
            source = self.root / "crates" / crate / "src" / "lib.rs"
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("pub fn pure() -> bool { true }\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def metadata(self, edges: dict[str, list[str]] | None = None) -> Path:
        selected = EXPECTED_EDGES if edges is None else edges
        packages = []
        for name in PACKAGE_NAMES:
            packages.append(
                {
                    "id": f"path+file:///{name}#0.3.0",
                    "name": name,
                    "dependencies": [
                        {"name": dependency} for dependency in selected[name]
                    ],
                }
            )
        metadata = {
            "packages": packages,
            "workspace_members": [package["id"] for package in packages],
        }
        path = self.root / "metadata.json"
        path.write_text(json.dumps(metadata), encoding="utf-8")
        return path

    def run_checker(self, metadata: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(CHECKER),
                "--root",
                str(self.root),
                "--metadata-file",
                str(metadata),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_valid_dependency_matrix_and_core_sources_pass(self) -> None:
        result = self.run_checker(self.metadata())

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Rust architecture checks passed", result.stdout)
        self.assertEqual(result.stderr, "")

    def test_upward_workspace_dependency_is_rejected(self) -> None:
        edges = {name: list(values) for name, values in EXPECTED_EDGES.items()}
        edges["sembla-runtime"] = ["sembla-cuda", "sembla-ir"]

        result = self.run_checker(self.metadata(edges))

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "sembla-runtime: workspace dependencies must be [sembla-ir]",
            result.stderr,
        )
        self.assertIn("sembla-cuda", result.stderr)

    def test_backend_vocabulary_and_reporting_macros_are_rejected(self) -> None:
        source = self.root / "crates" / "sembla-runtime" / "src" / "lib.rs"
        source.write_text(
            "// CUDA detail leaked downward\n"
            "pub fn noisy() { eprintln!(\"backend warning\"); }\n",
            encoding="utf-8",
        )

        result = self.run_checker(self.metadata())

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("backend-specific CUDA vocabulary is forbidden", result.stderr)
        self.assertIn("direct stdout/stderr reporting is forbidden", result.stderr)


if __name__ == "__main__":
    unittest.main()
