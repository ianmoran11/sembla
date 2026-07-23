#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest


CLEANER = Path(__file__).resolve().parents[1] / "clean-local.sh"
GIT_REDIRECTION_VARIABLES = (
    "GIT_INDEX_FILE",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_IMPLICIT_WORK_TREE",
)


class CleanLocalTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary_directory.name).resolve()
        self.root = self.base / "repository"
        self.root.mkdir()
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)

        script = self.root / "scripts" / "clean-local.sh"
        script.parent.mkdir()
        shutil.copy2(CLEANER, script)
        script.chmod(0o755)
        self.script = script

        self.caller_directory = self.base / "caller"
        self.caller_directory.mkdir()

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative_path: str, content: str = "sentinel\n") -> Path:
        path = self.root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def make_cache_tree(self) -> list[Path]:
        sentinels = [
            self.write("target/sentinel"),
            self.write("frontend/.lake/sentinel"),
            self.write(".pytest_cache/sentinel"),
            self.write("calibration/npe/.venv/sentinel"),
            self.write("calibration/npe/__pycache__/sentinel"),
            self.write("calibration/npe/package/__pycache__/sentinel"),
        ]
        return [path.parent for path in sentinels]

    def run_cleaner(
        self,
        *arguments: str,
        script: Path | None = None,
        environment: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        clean_environment = os.environ.copy()
        for variable in GIT_REDIRECTION_VARIABLES:
            clean_environment.pop(variable, None)
        if environment is not None:
            clean_environment.update(environment)

        return subprocess.run(
            ["bash", str(script or self.script), *arguments],
            cwd=self.caller_directory,
            env=clean_environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_default_is_dry_run_from_an_unrelated_cwd(self) -> None:
        cache_directories = self.make_cache_tree()
        protected = [
            self.write(".piprd/managed.json"),
            self.write(".pi-subagents/transcript.jsonl"),
            self.write("fixtures/golden.json"),
            self.write("examples/example.json"),
            self.write("calibration/npe/artifacts/posterior.pt"),
            self.write("calibration/npe/artifacts/__pycache__/sentinel"),
            self.write("spikes/precision/artifacts/evidence.txt"),
            self.write("spikes/precision/evidence/report.txt"),
            self.write("spikes/precision/infra/main.tf"),
            self.write("spikes/precision/infra-hyperstack/main.tf"),
            self.write("spikes/precision/infra-vultr/main.tf"),
        ]

        result = self.run_cleaner()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Dry run only; nothing will be deleted", result.stdout)
        self.assertIn("Rust and Lean outputs will need rebuilding", result.stdout)
        self.assertIn("Python dependencies may need reinstalling", result.stdout)
        for relative in (
            "target/",
            "frontend/.lake/",
            ".pytest_cache/",
            "calibration/npe/.venv/",
            "calibration/npe/__pycache__/",
            "calibration/npe/package/__pycache__/",
        ):
            self.assertIn(f"{relative}: exists (approximately ", result.stdout)
        for path in cache_directories + protected:
            self.assertTrue(path.exists(), path)
        for protected_text in (
            ".piprd",
            ".pi-subagents",
            "fixtures",
            "examples",
            "calibration/npe/artifacts",
            "spikes/precision",
            "terraform",
        ):
            self.assertNotIn(protected_text, result.stdout)
        self.assertEqual(result.stderr, "")

    def test_apply_removes_only_allowlisted_caches_and_is_idempotent(self) -> None:
        cache_directories = self.make_cache_tree()
        protected = [
            self.write(".piprd/managed.json"),
            self.write("fixtures/golden.json"),
            self.write("examples/example.json"),
            self.write("calibration/npe/artifacts/posterior.pt"),
            self.write("spikes/precision/evidence/report.txt"),
            self.write("spikes/precision/infra/main.tf"),
            self.write("spikes/precision/infra-hyperstack/main.tf"),
            self.write("spikes/precision/infra-vultr/main.tf"),
        ]
        outside = self.base / "outside-data"
        outside.mkdir()
        outside_sentinel = outside / "sentinel"
        outside_sentinel.write_text("outside\n", encoding="utf-8")
        os.symlink(outside, self.root / "target" / "external-link")

        result = self.run_cleaner("--apply")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Apply mode", result.stdout)
        for path in cache_directories:
            self.assertFalse(path.exists(), path)
        for path in protected:
            self.assertTrue(path.exists(), path)
        self.assertTrue(outside_sentinel.exists())

        repeated = self.run_cleaner("--apply")
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertIn("target/: missing", repeated.stdout)
        for path in protected:
            self.assertTrue(path.exists(), path)
        self.assertTrue(outside_sentinel.exists())

    def test_symlink_escape_is_refused(self) -> None:
        outside = self.base / "outside-data"
        outside.mkdir()
        outside_sentinel = outside / "sentinel"
        outside_sentinel.write_text("outside\n", encoding="utf-8")
        os.symlink(outside, self.root / "target")

        result = self.run_cleaner("--apply")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("escapes the repository root", result.stderr)
        self.assertTrue(outside_sentinel.exists())
        self.assertTrue((self.root / "target").is_symlink())

    def test_protected_denylist_is_refused(self) -> None:
        protected_directories = [
            ".git",
            ".piprd",
            ".pi-subagents",
            "fixtures",
            "examples",
            "calibration/npe/artifacts",
            "spikes/precision/artifacts",
            "spikes/precision/evidence",
            "spikes/precision/infra",
            "spikes/precision/infra-hyperstack",
            "spikes/precision/infra-vultr",
            "spikes/precision/terraform",
            "infrastructure/state.tfstate",
            "infrastructure/plan.tfplan",
            "infrastructure/config.tf",
        ]
        for relative in protected_directories:
            protected = self.root / relative
            protected.mkdir(parents=True, exist_ok=True)
            sentinel = protected / "sentinel"
            sentinel.write_text("protected\n", encoding="utf-8")
            target = self.root / "target"
            os.symlink(protected, target)

            with self.subTest(relative=relative):
                result = self.run_cleaner("--apply")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("protected path", result.stderr)
                self.assertTrue(sentinel.exists())
                self.assertTrue(target.is_symlink())

            target.unlink()

    def test_tracked_file_is_refused(self) -> None:
        tracked = self.write("target/tracked.txt")
        subprocess.run(
            ["git", "-C", str(self.root), "add", "target/tracked.txt"],
            check=True,
        )

        result = self.run_cleaner("--apply")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("contains tracked files", result.stderr)
        self.assertTrue(tracked.exists())

    def test_git_repository_and_index_overrides_cannot_hide_tracked_files(self) -> None:
        tracked = self.write("target/tracked.txt")
        subprocess.run(
            ["git", "-C", str(self.root), "add", "target/tracked.txt"],
            check=True,
        )
        alternate_git_directory = self.base / "alternate.git"
        subprocess.run(
            ["git", "init", "--quiet", "--bare", str(alternate_git_directory)],
            check=True,
        )
        overrides = [
            ("GIT_INDEX_FILE", {"GIT_INDEX_FILE": str(self.base / "alternate-index")}),
            (
                "GIT_DIR",
                {
                    "GIT_DIR": str(alternate_git_directory),
                    "GIT_WORK_TREE": str(self.root),
                },
            ),
            ("GIT_WORK_TREE", {"GIT_WORK_TREE": str(self.root)}),
            ("GIT_COMMON_DIR", {"GIT_COMMON_DIR": str(alternate_git_directory)}),
            ("GIT_IMPLICIT_WORK_TREE", {"GIT_IMPLICIT_WORK_TREE": "0"}),
        ]

        for variable, environment in overrides:
            with self.subTest(variable=variable):
                result = self.run_cleaner("--apply", environment=environment)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(
                    f"refusing Git repository/index override: {variable}",
                    result.stderr,
                )
                self.assertTrue(tracked.exists())

        help_result = self.run_cleaner(
            "--help",
            environment={"GIT_INDEX_FILE": str(self.base / "alternate-index")},
        )
        self.assertEqual(help_result.returncode, 0, help_result.stderr)

    def test_unknown_flag_fails_without_deleting(self) -> None:
        target = self.write("target/sentinel")

        result = self.run_cleaner("--force")

        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown option: --force", result.stderr)
        self.assertTrue(target.exists())

    def test_help_documents_opt_in_apply_without_deleting(self) -> None:
        target = self.write("target/sentinel")

        result = self.run_cleaner("--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Usage: bash scripts/clean-local.sh [--apply]", result.stdout)
        self.assertIn("The default is a dry run", result.stdout)
        self.assertTrue(target.exists())

    def test_symlinked_repository_root_is_refused(self) -> None:
        linked_root = self.base / "repository-link"
        os.symlink(self.root, linked_root)
        linked_script = linked_root / "scripts" / "clean-local.sh"

        result = self.run_cleaner("--apply", script=linked_script)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlinked repository root/script path", result.stderr)


if __name__ == "__main__":
    unittest.main()
