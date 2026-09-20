"""Exercise the verification command against disposable source trees."""

import json
import os
from pathlib import Path
import signal
import shlex
import subprocess
import sys
import tempfile
import unittest


COMMAND = Path(__file__).with_name("verification.py")


class VerificationCommandTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith("STORYOS_VERIFICATION_")}
        self.environment["PYTHONDONTWRITEBYTECODE"] = "1"
        self.git("init", "--quiet", "--initial-branch=main")
        self.git("config", "core.excludesFile", os.devnull)
        (self.root / "scripts").mkdir()
        (self.root / "docs/agents").mkdir(parents=True)
        (self.root / ".gitignore").write_text("target/\n")
        (self.root / "AGENTS.md").write_text("Fixture source.\n")
        policy = {"version": 1, "rules": [
            {"pattern": "*.md", "kind": "documentation", "group": "contracts"},
            {"pattern": ".gitignore", "kind": "configuration", "group": "complete"},
            {"pattern": "scripts/*", "kind": "verification", "group": "verification-tools"},
            {"pattern": "docs/*", "kind": "documentation", "group": "contracts"},
        ]}
        (self.root / "docs/agents/verification-policy.json").write_text(json.dumps(policy))
        self.git("add", ".")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "commit", "--quiet", "-m", "Create fixture.")

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True).strip()

    def cli(self, *args):
        return subprocess.run([sys.executable, str(COMMAND), *args], cwd=self.root,
                              capture_output=True, text=True, env=self.environment)

    def report(self):
        reports = list(self.root.glob("target/verification/*/report.json"))
        self.assertEqual(len(reports), 1)
        return json.loads(reports[0].read_text())

    def test_inventory_refuses_unknown_and_misplaced_test_files(self):
        self.assertEqual(self.cli("inventory", "--check").returncode, 0)
        (self.root / "surprise.xyz").write_text("unclassified")
        self.assertNotEqual(self.cli("inventory", "--check").returncode, 0)
        (self.root / "surprise.xyz").unlink()
        for name in ("misplaced.test.ts", "misplaced.spec.ts", "test_misplaced.py"):
            path = self.root / "scripts" / name
            path.write_text("unsupported test")
            self.assertNotEqual(self.cli("inventory", "--check").returncode, 0)
            path.unlink()

    def test_success_records_nested_stage_and_stable_source(self):
        result = self.cli("run", "--", sys.executable, str(COMMAND), "step", "sample",
                          "--", sys.executable, "-c", "print('visible child output')")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("visible child output", result.stdout)
        report = self.report()
        self.assertEqual(report["status"], "passed")
        self.assertEqual(report["source_start"], report["source_end"])
        self.assertEqual(report["source_start"]["commit"], self.git("rev-parse", "HEAD"))
        self.assertGreaterEqual(report["duration_seconds"], report["steps"][0]["duration_seconds"])
        self.assertEqual([(s["stage"], s["status"], s["exit_code"]) for s in report["steps"]],
                         [("sample", "passed", 0)])

    def test_child_failure_is_not_a_successful_report(self):
        result = self.cli("run", "--", sys.executable, str(COMMAND), "step", "failure",
                          "--", sys.executable, "-c", "raise SystemExit(7)")
        self.assertEqual(result.returncode, 7, result.stderr)
        report = self.report()
        self.assertEqual((report["status"], report["steps"][0]["exit_code"]), ("failed", 7))

    def test_source_edit_invalidates_a_successful_child(self):
        result = self.cli("run", "--", sys.executable, "-c",
                          "from pathlib import Path; Path('AGENTS.md').write_text('changed')")
        self.assertNotEqual(result.returncode, 0)
        report = self.report()
        self.assertEqual(report["status"], "source-changed")
        self.assertEqual(report["exit_code"], 0)
        self.assertNotEqual(report["source_start"], report["source_end"])

    def test_later_success_cannot_mask_a_failed_stage(self):
        failure = shlex.join([sys.executable, str(COMMAND), "step", "failure", "--",
                              sys.executable, "-c", "raise SystemExit(7)"])
        result = self.cli("run", "--", "sh", "-c", f"{failure}; true")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.report()["status"], "incomplete")

    def test_restoring_source_bytes_does_not_hide_an_intervening_write(self):
        mutation = ("from pathlib import Path; p=Path('AGENTS.md'); "
                    "original=p.read_bytes(); p.write_bytes(b'changed'); p.write_bytes(original)")
        result = self.cli("run", "--", sys.executable, str(COMMAND), "step", "mutation",
                          "--", sys.executable, "-c", mutation)
        self.assertNotEqual(result.returncode, 0)
        report = self.report()
        self.assertEqual(report["status"], "source-changed")
        self.assertFalse(report["source_end"]["dirty"])

    def test_no_executed_stages_cannot_produce_a_successful_report(self):
        result = self.cli("run", "--", sys.executable, "-c", "print('dry run')")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.report()["status"], "incomplete")

    @unittest.skipUnless(os.name == "posix", "Process-group interruption requires POSIX.")
    def test_interruption_is_reported_after_child_cleanup(self):
        child = ("import signal; "
                 "signal.signal(signal.SIGTERM, lambda *_: exit(0)); "
                 "print('ready', flush=True); "
                 "signal.pause()")
        with subprocess.Popen([sys.executable, str(COMMAND), "run", "--",
                               sys.executable, str(COMMAND), "step", "waiting", "--",
                               sys.executable, "-c", child], cwd=self.root,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              text=True, env=self.environment) as process:
            try:
                self.assertEqual(process.stdout.readline(), "ready\n")
                process.send_signal(signal.SIGTERM)
                process.communicate(timeout=10)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
            self.assertNotEqual(process.returncode, 0)
        self.assertEqual(self.report()["status"], "interrupted")
