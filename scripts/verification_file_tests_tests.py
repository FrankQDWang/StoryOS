"""Check file self-test interruption through the public targeted command."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import unittest

import verification_tests


RUNNER = Path(__file__).with_name("verification_test_files.py")


class FileInterruptionTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixture = verification_tests.VerificationCommandTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        self.root = root = fixture.root
        policy = root / "docs/agents/verification-policy.json"
        data = json.loads(policy.read_text())
        data["rules"].insert(0, {"pattern": "scripts/*_tests.py", "kind": "verification-test",
                                 "group": "verification-tools"})
        data["targeted"] = {"verification-tests": {"command": ["python3", "scripts/verification_test_files.py"],
                                                    "clean": False}}
        data["workflow"] = {"version": 1, "operations": {}, "stage_types": {},
                            "profiles": {"verification-tests": {"members": ["verification-tools"]},
                                         "verification-tools": {}},
                            "targeted": {"verification-tests": ["check:verification-tests"]}}
        data["verification_test_workers"] = 2
        data["verification_test_parallel_files"] = ["scripts/a_tests.py"]
        policy.write_text(json.dumps(data))
        (root / "scripts/verification_test_files.py").write_text(
            f"import runpy, sys\nsys.path.insert(0, {str(RUNNER.parent)!r})\n"
            f"runpy.run_path({str(RUNNER)!r}, run_name='__main__')\n")
        (root / "scripts/a_tests.py").write_text(
            "import os, signal, unittest\nclass Fixture(unittest.TestCase):\n"
            "    def test_wait(self):\n"
            "        print('FILE_READY ' + str(os.getpgrp()), flush=True)\n"
            "        signal.pause()\n")
        fixture.git("add", ".")
        fixture.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                    "commit", "--quiet", "-m", "Declare a file self-test.")

    def test_uncommitted_test_file_is_a_named_prerequisite_failure(self):
        (self.root / "scripts/b_tests.py").write_text("raise AssertionError('self-test ran')\n")
        result = self.fixture.cli("targeted", "--check", "verification-tests")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("The verification-tool self-tests require committed test files; "
                      "commit or remove these test files: scripts/b_tests.py\n", result.stderr)
        self.assertNotIn("FILE_READY", result.stdout)
        self.assertNotIn("Traceback", result.stderr)

    @unittest.skipUnless(os.name == "posix", "Process-group interruption requires POSIX.")
    def test_interruption_cleans_file_process_group(self):
        fixture, root = self.fixture, self.root
        with subprocess.Popen([sys.executable, str(verification_tests.COMMAND), "targeted", "--check",
                               "verification-tests"], cwd=root, env=fixture.environment,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as process:
            try:
                line = process.stdout.readline()
                self.assertTrue(line.startswith("FILE_READY "), line)
                group = int(line.split()[1])
                process.send_signal(signal.SIGINT)
                process.communicate()
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
        self.assertNotEqual(process.returncode, 0)
        with self.assertRaises(ProcessLookupError):
            os.killpg(group, 0)
        report_path = next(root.glob("target/verification/*/report.json"))
        self.assertEqual(json.loads(report_path.read_text())["status"], "interrupted")


if __name__ == "__main__":
    unittest.main()
