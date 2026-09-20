"""Observe daily reuse and resource admission through the public CLI."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import unittest

import verification_plan_tests


class DailyCacheTests(unittest.TestCase):
    def setUp(self):
        self.fixture = verification_plan_tests.FilePlanTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.install_runner_fixture()
        self.root = self.fixture.root
        ignore = self.root / ".gitignore"
        ignore.write_text(ignore.read_text() + "node_modules/\n")
        policy = json.loads(self.fixture.policy_path.read_text())
        policy["result_cache_profiles"] = ["node-contract"]
        self.fixture.policy_path.write_text(json.dumps(policy))
        self.fixture.repo.git("add", ".gitignore", "docs/agents/verification-policy.json")
        self.fixture.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                              "commit", "--quiet", "-m", "Ignore installed dependencies.")
        self.fixture.base = self.fixture.repo.git("rev-parse", "HEAD")
        tool = self.root / ".tools/pnpm"
        tool.write_text(tool.read_text().replace("output = next", "if '--version' in sys.argv:\n"
                        "    print('fixture-pnpm-1'); sys.exit(0)\noutput = next"))
        for folder in ("node_modules", "apps/web/node_modules"):
            directory = self.root / folder
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "installed.js").write_text("installed tool input")
        (self.root / "node_modules/workspace").symlink_to("../apps/web", target_is_directory=True)
        self.test = self.fixture.add_test()

    def run_daily(self, *args):
        result = self.fixture.cli("run", *args)
        reports = self.root.glob("target/verification/*/report.json")
        report = json.loads(max(reports, key=lambda path: path.stat().st_mtime_ns).read_text())
        return result, report

    def test_equal_inputs_reuse_without_child_execution_and_leaf_edits_invalidate(self):
        first, report = self.run_daily()
        self.assertEqual((first.returncode, report["cache"]["status"]), (0, "miss"), first.stderr)
        self.assertIn("executed selected files", first.stdout)
        for name in ("first.ts", "second.ts"):
            target = self.root / ".tools" / name
            target.write_text(self.test.read_text())
            self.test.unlink()
            self.test.symlink_to(target)
            result, report = self.run_daily()
            self.assertEqual((result.returncode, report["cache"]["status"]), (0, "miss"), result.stderr)
        second, report = self.run_daily()
        self.assertEqual((second.returncode, report["cache"]["status"]), (0, "hit"), second.stderr)
        self.assertNotIn("executed selected files", second.stdout)
        self.test.write_text(self.test.read_text() + "// changed input\n")
        changed, report = self.run_daily()
        self.assertEqual((changed.returncode, report["cache"]["status"]), (0, "miss"), changed.stderr)
        self.assertIn("executed selected files", changed.stdout)

    def test_new_commit_keeps_equal_inputs_but_membership_and_environment_do_not(self):
        self.assertEqual(self.run_daily()[0].returncode, 0)
        self.fixture.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                              "commit", "--quiet", "--allow-empty", "-m", "Record unrelated history.")
        result, report = self.run_daily()
        self.assertEqual((result.returncode, report["cache"]["status"]), (0, "hit"), result.stderr)
        self.fixture.add_test("another.test.ts")
        result, report = self.run_daily()
        self.assertEqual((result.returncode, report["cache"]["status"]), (0, "miss"), result.stderr)
        self.test.chmod(0o755)
        result, report = self.run_daily()
        self.assertEqual((result.returncode, report["cache"]["status"]), (0, "miss"), result.stderr)
        self.fixture.repo.environment["FIXTURE_INPUT"] = "changed environment"
        result, report = self.run_daily()
        self.assertEqual((result.returncode, report["cache"]["status"]), (0, "miss"), result.stderr)
        tool = self.root / ".tools/pnpm"
        tool.write_text(tool.read_text().replace("fixture-pnpm-1", "fixture-pnpm-2"))
        result, report = self.run_daily()
        self.assertEqual((result.returncode, report["cache"]["status"]), (0, "miss"), result.stderr)

    def test_missing_outputs_corrupt_entries_and_no_cache_force_execution(self):
        self.assertEqual(self.run_daily()[0].returncode, 0)
        entry = next(self.root.glob("target/verification-cache/*.json"))
        entry.write_text("{incomplete")
        result, report = self.run_daily()
        self.assertEqual((result.returncode, report["cache"]["status"]), (0, "miss"), result.stderr)
        self.assertIn("executed selected files", result.stdout)
        producer = self.root / json.loads(entry.read_text())["report"]
        producer.with_name("vitest.json").unlink()
        result, report = self.run_daily()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("executed selected files", result.stdout)
        (self.root / "node_modules/installed.js").chmod(0o755)
        result, report = self.run_daily()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("executed selected files", result.stdout)
        (self.root / "node_modules/installed.js").unlink()
        result, report = self.run_daily()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("executed selected files", result.stdout)
        result, report = self.run_daily("--no-cache")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["cache"]["status"], "bypass")
        self.assertIn("executed selected files", result.stdout)
        self.assertFalse(list(self.root.glob("target/verification-cache/*.json")))

    def test_rename_deletion_and_shared_changes_cannot_reuse_a_leaf_result(self):
        self.assertEqual(self.run_daily()[0].returncode, 0)
        reports = list(self.root.glob("target/verification/*/report.json"))

        def pending_plan():
            result = self.fixture.cli("run")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("pending", result.stderr)
            self.assertNotIn("executed selected files", result.stdout)
            self.assertEqual(list(self.root.glob("target/verification/*/report.json")), reports)
            return json.loads(self.fixture.cli("plan").stdout)

        self.fixture.repo.git("add", str(self.test))
        self.fixture.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                              "commit", "--quiet", "-m", "Register the test file.")
        renamed = self.test.with_name("renamed.test.ts")
        self.fixture.repo.git("mv", str(self.test), str(renamed))
        self.assertEqual(pending_plan()["checks"][0]["group"], "complete")
        renamed.unlink()
        self.assertNotIn(str(renamed.relative_to(self.root)), pending_plan()["test_files"])
        (self.root / "AGENTS.md").write_text("Changed shared policy input.\n")
        self.assertEqual(pending_plan()["checks"][0]["group"], "complete")

    def test_failure_and_source_changes_do_not_publish_reusable_results(self):
        self.test.write_text(self.test.read_text() + "// NO_TESTS\n")
        for _ in range(2):
            result, report = self.run_daily()
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(report["status"], "failed")
            self.assertFalse(list(self.root.glob("target/verification-cache/*.json")))
        self.test.write_text(self.test.read_text().replace("NO_TESTS", "test input"))
        tool = self.root / ".tools/pnpm"
        tool.write_text(tool.read_text().replace("results =", "files[0].write_text(files[0].read_text() + '// drift\\n')\nresults ="))
        result, report = self.run_daily()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(report["status"], "source-changed")
        self.assertFalse(list(self.root.glob("target/verification-cache/*.json")))
        original = tool.read_text()
        for mutation in ("pathlib.Path('node_modules/installed.js').write_text('changed dependency')",
                         "link = pathlib.Path('node_modules/workspace'); link.unlink(); link.symlink_to('../apps/web')"):
            tool.write_text(original.replace("files[0].write_text(files[0].read_text() + '// drift\\n')", mutation))
            result, report = self.run_daily()
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(list(self.root.glob("target/verification-cache/*.json")))

    def test_busy_budget_refuses_a_second_run_and_releases_after_interruption(self):
        target = self.root / "target"
        target.mkdir()
        fifo = target / "release"
        os.mkfifo(fifo)
        tool = self.root / ".tools/pnpm"
        original = tool.read_text()
        tool.write_text(original.replace("output = next", """import os, signal
if os.environ.get('FIXTURE_WAIT'):
    def cleanup(*_):
        print('FIXTURE_CLEANUP', flush=True)
        pathlib.Path('target/release').read_text()
        sys.exit(0)
    signal.signal(signal.SIGTERM, cleanup)
    print(f'FIXTURE_READY {os.getpgrp()}', flush=True)
    signal.pause()
output = next"""))
        self.fixture.repo.git("add", str(self.test))
        self.fixture.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                              "commit", "--quiet", "-m", "Prepare a clean command fixture.")
        command = [sys.executable, str(Path(__file__).with_name("verification.py")), "run", "--",
                   sys.executable, "-c", "import subprocess,sys,signal; "
                   "subprocess.Popen([sys.executable, '.tools/pnpm']); signal.pause()"]
        child = subprocess.Popen(command, cwd=self.root, env={**self.fixture.repo.environment, "FIXTURE_WAIT": "1"},
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        group = None
        try:
            for line in child.stdout:
                if line.startswith("FIXTURE_READY "):
                    group = int(line.split()[1])
                    break
            self.assertIsNone(child.poll())
            result = self.fixture.repo.cli("run", "--", sys.executable, "-c", "print('SECOND_CHILD_EXECUTED')")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("budget is busy", result.stderr)
            child.terminate()
            cleanup = waiting = False
            for line in child.stdout:
                cleanup |= line.strip() == "FIXTURE_CLEANUP"
                waiting |= line.strip() == "Waiting for verification child cleanup"
                if line.startswith("Verification interrupted:"):
                    child.wait(timeout=10)
                    break
                if cleanup and waiting:
                    break
            result = self.fixture.repo.cli("run", "--", sys.executable, "-c", "print('SECOND_CHILD_EXECUTED')")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("budget is busy", result.stderr)
            fifo.write_text("cleanup may finish")
            child.communicate(timeout=10)
        finally:
            if group is not None:
                try:
                    os.killpg(group, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            if child.poll() is None:
                child.kill()
            child.communicate()
        self.assertFalse(list(self.root.glob("target/verification-cache/*.json")))
        tool.write_text(original)
        result, report = self.run_daily("--workers", "1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["budget"], {"groups": 1, "workers": 1})
        self.assertNotEqual(self.fixture.cli("run", "--workers", "999").returncode, 0)
