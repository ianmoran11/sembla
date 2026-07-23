#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check-markdown-links.py"


class MarkdownLinkCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        subprocess.run(
            ["git", "init", "--quiet", str(self.root)],
            check=True,
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative_path: str, content: str) -> None:
        path = self.root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        subprocess.run(
            ["git", "-C", str(self.root), "add", "-A"],
            check=True,
        )
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_valid_encoded_fragment_and_spaced_links_ignore_non_links(self) -> None:
        self.write("docs/target file.md", "# Existing target\n")
        self.write(
            "docs/source.md",
            """# Links

[angle destination](<target file.md>)
[URL-encoded destination](target%20file.md#existing-target)
[pure fragment](#links)
[remote](https://example.com/missing.md)
[email](mailto:security@example.com)
`[inline code](inline-missing.md)`

```markdown
[fenced example](fenced-missing.md)
```

```markdown
```not-a-closing-fence
[still fenced](also-fenced-missing.md)
```

![missing image is not a documentation link](missing-image.png)
""",
        )
        self.write(
            ".piprd/managed.md",
            "[managed runtime link](managed-missing.md)\n",
        )

        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("checked 2 local links in 2 tracked Markdown files", result.stdout)
        self.assertEqual(result.stderr, "")

    def test_missing_target_reports_sorted_file_and_line_evidence(self) -> None:
        self.write(
            "docs/source.md",
            "# Missing\n\n[first](z-missing.md)\n[second](a%20missing.md#part)\n",
        )
        self.write("README.md", "[also missing](not-there.md)\n")

        result = self.run_checker()

        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertEqual(
            result.stderr.splitlines(),
            [
                "README.md:1 -> not-there.md",
                "docs/source.md:3 -> z-missing.md",
                "docs/source.md:4 -> a%20missing.md#part",
                "error: 3 missing local Markdown target(s)",
            ],
        )


if __name__ == "__main__":
    unittest.main()
