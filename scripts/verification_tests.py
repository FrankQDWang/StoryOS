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

import verification_cache
import verification_summary


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
        self.assertNotIn("failed_steps", report)
        report_path = Path(report["repository"]) / "target/verification" / report["run_id"] / "report.json"
        self.assertEqual(result.stdout.splitlines()[-1],
                         f"Verification passed: {report['duration_seconds']:.2f}s; report: {report_path}")

    def test_child_failure_is_not_a_successful_report(self):
        result = self.cli("run", "--", sys.executable, str(COMMAND), "step", "outer", "--",
                          sys.executable, str(COMMAND), "step", "failure", "--",
                          sys.executable, "-c", "print('failure output'); raise SystemExit(7)")
        self.assertEqual(result.returncode, 7, result.stderr)
        report = self.report()
        inner = next(step for step in report["steps"] if step["stage"] == "failure")
        reason = "The failure step stopped with status failed and exit code 7."
        self.assertEqual({key: report[key] for key in ("status", "failed_steps", "failure_reason", "failure_log")},
                         {"status": "failed", "failed_steps": ["failure"], "failure_reason": reason,
                          "failure_log": inner["log"]})
        self.assertEqual(Path(inner["log"]).read_text(), "failure output\n")
        report_path = Path(report["repository"]) / "target/verification" / report["run_id"] / "report.json"
        self.assertEqual(result.stdout.splitlines()[-1],
                         f"Verification failed: {report['duration_seconds']:.2f}s; failed step: failure; "
                         f"reason: {reason[:-1]}; log: {inner['log']}; report: {report_path}")

    def test_observation_storage_failure_does_not_replace_test_result(self):
        shared = self.root / '.git/storyos-observation'
        shared.mkdir()
        (shared / 'records').write_text('Unavailable projection storage')
        result = self.cli('step', 'sample', '--', sys.executable, '-c', 'print("executed")')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Observation publication failed', result.stderr)
        self.assertEqual(self.report()['status'], 'passed')

    def test_source_edit_invalidates_a_successful_child(self):
        result = self.cli("run", "--", sys.executable, "-c",
                          "from pathlib import Path; Path('AGENTS.md').write_text('changed')")
        self.assertNotEqual(result.returncode, 0)
        report = self.report()
        reason = "Verification inputs changed during the run: AGENTS.md."
        self.assertEqual({key: report[key] for key in ("status", "exit_code", "failed_steps", "failure_reason",
                                                       "changed_paths", "failure_log")},
                         {"status": "source-changed", "exit_code": 0, "failed_steps": [], "failure_reason": reason,
                          "changed_paths": ["AGENTS.md"], "failure_log": report["log"]})
        self.assertIn(f"failed step: none; reason: {reason[:-1]}; log: {report['log']}", result.stdout.splitlines()[-1])
        self.assertNotEqual(report["source_start"], report["source_end"])

    def test_stage_budget_fails_even_when_child_handles_termination_as_success(self):
        policy_path = self.root / 'docs/agents/verification-policy.json'
        policy = json.loads(policy_path.read_text())
        policy['stage_budgets_seconds'] = {'waiting': 1}
        policy_path.write_text(json.dumps(policy))
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '--quiet', '-m', 'Set stage budget.')
        child = 'import signal; signal.signal(signal.SIGTERM, lambda *_: exit(0)); signal.pause()'
        result = self.cli('step', 'waiting', '--', sys.executable, '-c', child)
        self.assertNotEqual(result.returncode, 0)
        report = self.report()
        self.assertEqual(report['steps'][0]['budget_exceeded'], True)
        self.assertEqual(report['steps'][0]['exit_code'], 124)

    def test_new_worktree_publishes_records_to_the_same_read_copy_directory(self):
        self.assertEqual(self.cli('step', 'sample', '--', sys.executable, '-c', 'pass').returncode, 0)
        first = self.report()
        other = self.root / 'target/other'
        self.git('worktree', 'add', '--detach', str(other), 'HEAD')
        result = subprocess.run([sys.executable, str(COMMAND), 'step', 'sample', '--',
                                 sys.executable, '-c', 'pass'], cwd=other,
                                env=self.environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        copies = list((self.root / '.git/storyos-observation/records').glob('*/report.json'))
        self.assertEqual(len(copies), 2)
        self.assertEqual({json.loads(p.read_text())['repository'] for p in copies},
                         {str(self.root.resolve()), str(other.resolve())})
        original = self.root / 'target/verification' / first['run_id'] / 'report.json'
        copied = self.root / '.git/storyos-observation/records' / first['run_id'] / 'report.json'
        self.assertEqual(original.read_bytes(), copied.read_bytes())

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

    def test_whitespace_step_checks_tracked_and_untracked_files_from_the_merge_base(self):
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        script = Path(__file__).with_name("verify-diff-whitespace.sh")
        environment = {key: value for key, value in self.environment.items() if key != "STORYOS_PR_BASE_SHA"}
        for path in (self.root / "AGENTS.md", self.root / "docs/untracked.md"):
            original = path.read_text() if path.exists() else "Fixture source.\n"
            path.write_text(original + "\n")
            failure = subprocess.run(["sh", str(script)], cwd=self.root, env=environment,
                                     capture_output=True, text=True)
            self.assertEqual((failure.returncode, failure.stdout.strip()),
                             (2, f"{path.relative_to(self.root)}:2: new blank line at EOF."))
            path.write_text(original)
            fixed = subprocess.run(["sh", str(script)], cwd=self.root, env=environment,
                                   capture_output=True, text=True)
            self.assertEqual(fixed.returncode, 0, fixed.stdout + fixed.stderr)
        self.assertEqual(self.git("status", "--porcelain"), "?? docs/untracked.md")

    def test_no_executed_stages_cannot_produce_a_successful_report(self):
        result = self.cli("run", "--", sys.executable, "-c", "print('dry run')")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.report()["status"], "incomplete")

    def test_unchanged_tool_inputs_reuse_a_passed_self_test_result(self):
        child = "from pathlib import Path; p = Path('target/launches'); p.write_text(p.read_text() + 'x' if p.exists() else 'x')"
        runs = []

        def run():
            result = self.cli("step", "verification-tests", "--", sys.executable, "-c", child)
            self.assertEqual(result.returncode, 0, result.stderr)
            report = max((json.loads(path.read_text()) for path in self.root.glob("target/verification/*/report.json")),
                         key=lambda item: item["started_at"])
            step = next(item for item in report["steps"] if item["stage"] == "verification-tests")
            runs.append((report, step, (self.root / "target/launches").read_text()))

        run()
        run()
        (self.root / "AGENTS.md").write_text("Edit outside the reuse key.\n")
        run()
        (self.root / "scripts/tool.py").write_text("Edit inside the reuse key.\n")
        run()
        run()
        self.environment["STORYOS_VERIFICATION_TEST_WORKERS"] = "1"
        run()
        self.assertEqual([(report["status"], step["status"], launches) for report, step, launches in runs],
                         [("passed", "passed", "x"), ("passed", "cached", "x"), ("passed", "cached", "x"),
                          ("passed", "passed", "xx"), ("passed", "cached", "xx"), ("passed", "passed", "xxx")])
        producer, executed, _ = runs[3]
        self.assertEqual({key: runs[4][1].get(key) for key in ("reuse_key", "producer", "producer_step")},
                         {"reuse_key": executed["reuse_key"], "producer_step": executed["id"],
                          "producer": f"target/verification/{producer['run_id']}/report.json"})
        self.assertNotEqual(executed["reuse_key"], runs[0][1]["reuse_key"])
        self.assertNotIn("reuse_key", runs[5][1])

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
                process.communicate()
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
            self.assertNotEqual(process.returncode, 0)
        self.assertEqual(self.report()["status"], "interrupted")

    def queue_fixture(self):
        policy_path = self.root / "docs/agents/verification-policy.json"
        policy = json.loads(policy_path.read_text())
        policy["complete"] = {"stages": ["heavy", "light"], "groups": {}, "host_queue": ["heavy"]}
        policy_path.write_text(json.dumps(policy))
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "commit", "--quiet", "-am", "Queue the heavy stage.")
        outside = tempfile.TemporaryDirectory()
        self.addCleanup(outside.cleanup)
        second = Path(outside.name) / "second"
        self.git("worktree", "add", "--quiet", "--detach", str(second))
        return second, Path(outside.name)

    def start(self, tree, stage, script):
        return subprocess.Popen([sys.executable, str(COMMAND), "run", "--", sys.executable, str(COMMAND), "step",
                                 stage, "--", sys.executable, "-c", script], cwd=tree, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, text=True, env=self.environment)

    def reports(self, tree):
        return sorted((json.loads(path.read_text()) for path in tree.glob("target/verification/*/report.json")),
                      key=lambda report: report["started_at"])

    def test_heavy_stages_in_two_worktrees_serialize_and_light_stages_do_not_wait(self):
        second, outside = self.queue_fixture()
        release = outside / "release"
        finished = outside / "first-finished"
        holder_script = ("import pathlib, time; print('FIRST_HOLDS', flush=True)\n"
                         f"while not pathlib.Path({str(release)!r}).exists(): time.sleep(0.05)\n"
                         f"pathlib.Path({str(finished)!r}).write_text('done')")
        second_script = ("import pathlib, sys; print('SECOND_RUNS', flush=True); "
                         f"sys.exit(0 if pathlib.Path({str(finished)!r}).exists() else 3)")
        first = self.start(self.root, "heavy", holder_script)
        waiting = None
        try:
            self.assertEqual(first.stdout.readline(), "FIRST_HOLDS\n")
            holder = verification_cache.queue_state(second)
            self.assertEqual({k: holder[k] for k in ("pid", "worktree", "stage")},
                             {"pid": holder["pid"], "worktree": str(self.root.resolve()), "stage": "heavy"})
            light = subprocess.run([sys.executable, str(COMMAND), "run", "--", sys.executable, str(COMMAND), "step",
                                    "light", "--", sys.executable, "-c", "print('LIGHT_RUNS')"], cwd=second,
                                   capture_output=True, text=True, env=self.environment, timeout=60)
            self.assertEqual(light.returncode, 0, light.stderr)
            self.assertNotIn("host_queue", self.reports(second)[0])
            waiting = self.start(second, "heavy", second_script)
            line = waiting.stdout.readline()
            self.assertTrue(line.startswith("Host queue: waiting"), line)
            self.assertIn(f"stage heavy of worktree {self.root.resolve()}", line)
            self.assertIsNone(waiting.poll())
            release.write_text("done")
            self.assertEqual(first.wait(timeout=60), 0)
            output, errors = waiting.communicate(timeout=60)
            self.assertEqual(waiting.returncode, 0, errors)
            self.assertIn("SECOND_RUNS", output)
        finally:
            for process in (first, waiting):
                if process and process.poll() is None:
                    process.kill()
                if process:
                    process.communicate()
        held = self.reports(self.root)[0]
        queued = self.reports(second)[-1]
        self.assertEqual(held["host_queue"][0]["released"], None)
        self.assertGreater(queued["host_queue"][0]["waited_seconds"], 0)
        self.assertEqual(verification_cache.queue_state(second), "free")
        self.assertEqual(verification_summary.summary({"run_id": "attempt", "hostQueue": "free"})["hostQueue"], "free")

    def test_a_holder_without_a_process_is_released_and_recorded(self):
        second, outside = self.queue_fixture()
        gone = subprocess.Popen([sys.executable, "-c", "pass"])
        gone.wait()
        holder = {"pid": gone.pid, "worktree": "/removed", "stage": "heavy", "started_at": "2026-10-06T00:00:00+00:00"}
        lock = verification_cache.queue_path(self.root)
        lock.write_text(json.dumps(holder))
        self.assertEqual(verification_cache.queue_state(second), "free")
        self.assertEqual(verification_summary.summary({"run_id": "attempt", "hostQueue": holder})["hostQueue"], holder)
        release = outside / "release"
        first = self.start(self.root, "heavy", "import pathlib, time; print('FIRST_HOLDS', flush=True)\n"
                           f"while not pathlib.Path({str(release)!r}).exists(): time.sleep(0.05)")
        waiting = None
        try:
            output = first.stdout.readline()
            self.assertEqual(output, "Host queue: released the lock of stage heavy of worktree /removed, started "
                             f"2026-10-06T00:00:00+00:00, process {gone.pid}; that process ended without a release\n")
            self.assertEqual(first.stdout.readline(), "FIRST_HOLDS\n")
            waiting = self.start(second, "heavy", "print('SECOND_RUNS', flush=True)")
            line = waiting.stdout.readline()
            self.assertIn(f"stage heavy of worktree {self.root.resolve()}", line)
            release.write_text("done")
            self.assertEqual(first.wait(timeout=60), 0)
            output, errors = waiting.communicate(timeout=60)
            self.assertEqual(waiting.returncode, 0, errors)
            self.assertNotIn("released the lock", output)
        finally:
            for process in (first, waiting):
                if process and process.poll() is None:
                    process.kill()
                if process:
                    process.communicate()
        self.assertEqual(self.reports(self.root)[-1]["host_queue"][0]["released"], holder)
        self.assertEqual(self.reports(second)[-1]["host_queue"][0]["released"], None)
        self.assertEqual(lock.read_text(), "")
