#!/usr/bin/env python3
"""Tests of the checks of the Linux package in scripts/package-release.py (issue 1854): a failed check fails the
command.

Each test makes a package of shell scripts in the place of the three binaries, so the test needs no Linux build.
"""

import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
SPEC = importlib.util.spec_from_file_location("package_release", ROOT / "scripts/package-release.py")
PACKAGE_RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE_RELEASE)
PASSING = {
    "storyos-server": "exit 0",
    "storyos-worker": "exit 0",
    "storyos-storage": "echo STORYOS_STORAGE_ACTIVATION=Active",
}


def check(binaries, limit=120):
    """Return the printed lines and the refusal of the check, or None when the check passes."""
    printed = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="linux-package-", dir=ROOT / "target") as package:
        (Path(package) / "web").mkdir()
        for name, body in binaries.items():
            path = Path(package) / name
            path.write_text(f"#!/bin/sh\n{body}\n")
            path.chmod(0o755)
        try:
            with contextlib.redirect_stdout(printed):
                PACKAGE_RELEASE.check_linux_package(package, limit)
        except SystemExit as refusal:
            return printed.getvalue(), str(refusal)
    return printed.getvalue(), None


def package_containers():
    names = subprocess.check_output(["docker", "ps", "--all", "--format", "{{.Names}}"], text=True,
                                    timeout=60).split()
    return {name for name in names if name.startswith("storyos-linux-package-")}


class CheckLinuxPackageTests(unittest.TestCase):
    def setUp(self):
        self.containers_before = package_containers()

    def tearDown(self):
        self.assertEqual(package_containers(), self.containers_before)

    def test_a_package_that_passes_each_check_passes(self):
        printed, refusal = check(PASSING)
        self.assertIsNone(refusal, printed)
        self.assertIn("The Linux package passed its checks", printed)

    def test_each_failed_check_fails_the_command(self):
        failures = {
            "storyos-server": "exit 1",
            "storyos-worker": "exit 1",
            "storyos-storage": "echo 'storage activation refused' >&2; exit 1",
        }
        for name, body in failures.items():
            with self.subTest(binary=name):
                printed, refusal = check({**PASSING, name: body})
                self.assertIn("failed with the status 1", refusal)
                self.assertNotIn("The Linux package passed its checks", printed)

    def test_a_check_that_does_not_end_fails_the_command(self):
        printed, refusal = check({**PASSING, "storyos-worker": "sleep 600"}, limit=5)
        self.assertEqual(refusal, "docker run did not end in 5 s")
        self.assertNotIn("The Linux package passed its checks", printed)

    def test_a_storage_run_that_does_not_report_active_fails_the_command(self):
        printed, refusal = check({**PASSING, "storyos-storage": "echo STORYOS_STORAGE_ACTIVATION=Refused"})
        self.assertEqual(refusal, "storyos-storage did not activate the database")
        self.assertNotIn("The Linux package passed its checks", printed)


if __name__ == "__main__":
    unittest.main()
