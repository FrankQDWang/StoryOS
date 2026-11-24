#!/usr/bin/env python3
"""Tests of scripts/check-linux-package.sh (issue 1854): a failed check of the Linux package fails the command.

Each test makes a package of shell scripts in the place of the three binaries, so the test needs no Linux build.
"""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
PASSING = {
    "storyos-server": "exit 0",
    "storyos-worker": "exit 0",
    "storyos-storage": "echo STORYOS_STORAGE_ACTIVATION=Active",
}


def check(binaries, check_seconds=120):
    with tempfile.TemporaryDirectory(prefix="linux-package-", dir=ROOT / "target") as package:
        (Path(package) / "web").mkdir()
        for name, body in binaries.items():
            path = Path(package) / name
            path.write_text(f"#!/bin/sh\n{body}\n")
            path.chmod(0o755)
        return subprocess.run([str(ROOT / "scripts/check-linux-package.sh"), package], cwd=ROOT,
                              env={**os.environ, "STORYOS_LINUX_PACKAGE_CHECK_SECONDS": str(check_seconds)},
                              stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=600)


def package_containers():
    names = subprocess.check_output(["docker", "ps", "--all", "--format", "{{.Names}}"], text=True).split()
    return {name for name in names if name.startswith("storyos-linux-package-")}


class CheckLinuxPackageTests(unittest.TestCase):
    def test_a_package_that_passes_each_check_passes(self):
        result = check(PASSING)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("The Linux package passed its checks", result.stdout)

    def test_each_failed_check_fails_the_command(self):
        failures = {
            "storyos-server": "exit 1",
            "storyos-worker": "exit 1",
            "storyos-storage": "echo 'storage activation refused' >&2; exit 1",
        }
        for name, body in failures.items():
            with self.subTest(binary=name):
                result = check({**PASSING, name: body})
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn("The Linux package passed its checks", result.stdout)

    def test_a_check_that_does_not_end_fails_the_command_and_removes_its_container(self):
        before = package_containers()
        result = check({**PASSING, "storyos-worker": "sleep 600"}, check_seconds=5)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("The Linux package passed its checks", result.stdout)
        self.assertEqual(package_containers(), before)

    def test_a_storage_run_that_does_not_report_active_fails_the_command(self):
        result = check({**PASSING, "storyos-storage": "echo STORYOS_STORAGE_ACTIVATION=Refused"})
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("storyos-storage did not activate the database", result.stderr)


if __name__ == "__main__":
    unittest.main()
