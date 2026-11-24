#!/usr/bin/env python3
"""Tests of the checks of the Linux package in scripts/package-release.py (issue 1854): a failed check fails the
command.

Each test makes a package of shell scripts in the place of the three binaries, so the test needs no Linux build.
"""

import contextlib
import importlib.util
import io
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
SPEC = importlib.util.spec_from_file_location("package_release", ROOT / "scripts/package-release.py")
PACKAGE_RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE_RELEASE)
PASSING = {
    "storyos-server": "exit 0",
    "storyos-worker": "exit 0",
    "storyos-storage": "echo STORYOS_STORAGE_ACTIVATION=Active",
}


def write_package(package, binaries):
    (Path(package) / "web").mkdir()
    for name, body in binaries.items():
        path = Path(package) / name
        path.write_text(f"#!/bin/sh\n{body}\n")
        path.chmod(0o755)


def check(binaries, limit=120):
    """Return the printed lines and the refusal of the check, or None when the check passes."""
    printed = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="linux-package-", dir=ROOT / "target") as package:
        write_package(package, binaries)
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

    def test_a_stop_signal_stops_the_check_and_removes_its_containers(self):
        with tempfile.TemporaryDirectory(prefix="linux-package-", dir=ROOT / "target") as package:
            write_package(package, {**PASSING, "storyos-worker": "sleep 600"})
            script = (
                "import importlib.util, signal, sys\n"
                "spec = importlib.util.spec_from_file_location('package_release', sys.argv[1])\n"
                "module = importlib.util.module_from_spec(spec)\n"
                "spec.loader.exec_module(module)\n"
                # The handler of main(); a module with no handler gets the default action.
                "signal.signal(signal.SIGTERM, getattr(module, 'stop_on_signal', signal.SIG_DFL))\n"
                "module.check_linux_package(sys.argv[2])\n"
            )
            with subprocess.Popen([sys.executable, "-c", script, str(ROOT / "scripts/package-release.py"), package],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) as process:
                worker = f"storyos-linux-package-{process.pid}-worker"
                deadline = time.monotonic() + 60
                while worker not in package_containers():
                    self.assertLess(time.monotonic(), deadline, "the Worker check did not start")
                    time.sleep(0.2)
                process.send_signal(signal.SIGTERM)
                self.assertEqual(process.wait(timeout=90), 128 + signal.SIGTERM)

    def test_a_failed_container_removal_fails_the_check(self):
        refused = subprocess.CompletedProcess([], 1, stdout="", stderr="Error response from daemon: refused")
        with mock.patch.object(PACKAGE_RELEASE.subprocess, "run", return_value=refused):
            with self.assertRaises(SystemExit) as refusal:
                PACKAGE_RELEASE.remove_containers("storyos-linux-package-0")
        self.assertIn("could not remove storyos-linux-package-0", str(refusal.exception))


class WriteArchiveTests(unittest.TestCase):
    def test_a_failed_move_of_the_checksum_leaves_no_archive_with_another_checksum(self):
        commit = "0123456789ab" + "0" * 28
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            package = output / "package"
            package.mkdir()
            (package / "storyos-server").write_text("new")
            archive = output / "storyos-linux-x86_64-0123456789ab.tar.gz"
            checksum = output / "storyos-linux-x86_64-0123456789ab.tar.gz.sha256"
            archive.write_text("old")
            checksum.write_text(f"{'0' * 64}  {archive.name}\n")
            replace = Path.replace

            def refuse_checksum(path, destination):
                if Path(destination).name.endswith(".sha256"):
                    raise OSError("refused")
                return replace(path, destination)

            with mock.patch.object(PACKAGE_RELEASE, "target", output), \
                    mock.patch.object(Path, "replace", refuse_checksum), self.assertRaises(OSError):
                PACKAGE_RELEASE.write_archive(package, commit)
            self.assertEqual((archive.exists(), checksum.exists()), (False, False))


if __name__ == "__main__":
    unittest.main()
