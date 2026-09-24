"""Observe candidate evidence acceptance through repository commands."""

import base64
import copy
import gzip
import json
from pathlib import Path
import subprocess
import sys
import unittest

import verification_tests


COMMAND = Path(__file__).with_name("verification_evidence.py")


class CandidateEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.fixture = verification_tests.VerificationCommandTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.fixture.environment["PYTHONDONTWRITEBYTECODE"] = "1"
        (self.root / "scripts/sample_tests.py").write_text("import unittest\nclass Sample(unittest.TestCase):\n    def test_child(self): pass\n")
        policy = self.root / "docs/agents/verification-policy.json"
        data = json.loads(policy.read_text())
        data["complete"] = {"stages": ["sample"], "groups": {"verification-tools": ["sample"]}}
        data["rules"].insert(0, {"pattern": "scripts/*_tests.py", "kind": "verification-test", "group": "verification-tools"})
        data["rules"].append({"pattern": "Makefile", "kind": "verification", "group": "complete"})
        policy.write_text(json.dumps(data))
        (self.root / "Makefile").write_text(f"verify-local-steps:\n\t{sys.executable} {verification_tests.COMMAND} step sample -- python3 -m unittest discover -s scripts -p '*_tests.py'\n")
        self.commit()
        self.base = self.fixture.git("rev-parse", "HEAD")
        self.fixture.git("update-ref", "refs/remotes/origin/main", self.base)

    def commit(self):
        self.fixture.git("add", ".")
        self.fixture.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "Record fixture inputs.")

    def cli(self, *arguments):
        return subprocess.run([sys.executable, str(COMMAND), *arguments], cwd=self.root,
                              env=self.fixture.environment, capture_output=True, text=True)

    def prepare(self, *arguments):
        run = self.fixture.cli("run", "--", "make", "verify-local-steps")
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        report = max(self.root.glob("target/verification/*/report.json"), key=lambda p: p.stat().st_mtime_ns)
        result = self.cli("prepare", "--report", str(report), "--head", self.fixture.git("rev-parse", "HEAD"),
                          "--base", self.base, "--baseline", self.base, *arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.body = self.root / "target/evidence.txt"
        self.body.write_text(result.stdout)
        return report

    def check(self):
        return self.cli("check", "--evidence", str(self.body), "--candidate", "HEAD", "--baseline", self.base,
                        "--head", self.fixture.git("rev-parse", "HEAD"), "--base", self.base)

    def write_report(self, report):
        prefix, value = self.body.read_text().split("\n", 1)
        packet = json.loads(value)
        packet["report"] = base64.b64encode(gzip.compress(json.dumps(report).encode())).decode()
        self.body.write_text(prefix + "\n" + json.dumps(packet))

    def test_current_complete_report_is_accepted_without_a_new_commit(self):
        head = self.fixture.git("rev-parse", "HEAD")
        self.prepare()
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Candidate evidence passed", result.stdout)
        self.assertEqual(self.fixture.git("rev-parse", "HEAD"), head)

    def test_malformed_success_records_do_not_count_as_execution(self):
        original = json.loads(self.prepare().read_text())
        for field, value in (("version", True), ("duration_seconds", False)):
            report = copy.deepcopy(original)
            report[field] = value
            self.write_report(report)
            self.assertNotEqual(self.check().returncode, 0, field)
        report = copy.deepcopy(original)
        report["steps"][0]["command"] = ["python3", "-c", "print(42)"]
        self.write_report(report)
        self.assertNotEqual(self.check().returncode, 0)
        policy = self.root / "docs/agents/verification-policy.json"
        data = json.loads(policy.read_text())
        data["complete"]["groups"]["verification-tools"] = "sample"
        policy.write_text(json.dumps(data))
        self.assertNotEqual(self.fixture.cli("inventory", "--check").returncode, 0)

    def test_worker_policy_requires_complete_graph_bound_attempts(self):
        policy = self.root / "docs/agents/verification-policy.json"
        data = json.loads(policy.read_text())
        data["verification_test_workers"] = 1
        data["verification_test_parallel_files"] = []
        data["complete"] = {"stages": ["verification-tests"],
                            "groups": {"verification-tools": ["verification-tests"]}}
        data["workflow"] = {"version": 1, "operations": {}, "stage_types": {},
                            "profiles": {"verification-tests": {"members": ["verification-tools"]},
                                         "verification-tools": {}}, "targeted": {}}
        policy.write_text(json.dumps(data))
        source = Path(__file__).with_name("verification_test_files.py")
        (self.root / "scripts/verification_test_files.py").write_text(
            f"import runpy, sys\nsys.path.insert(0, {str(source.parent)!r})\n"
            f"runpy.run_path({str(source)!r}, run_name='__main__')\n")
        (self.root / "Makefile").write_text(
            f"verify-local-steps:\n\t{sys.executable} {verification_tests.COMMAND} step verification-tests -- "
            "python3 scripts/verification_test_files.py\n")
        self.commit()
        self.base = self.fixture.git("rev-parse", "HEAD")
        self.fixture.git("update-ref", "refs/remotes/origin/main", self.base)
        report = json.loads(self.prepare().read_text())
        self.assertEqual(self.check().returncode, 0, self.check().stderr)
        self.assertEqual(len(report["verification_test_file_attempts"]), 1)
        attempt = report["verification_test_file_attempts"][0]
        for attempts in ([], [attempt, attempt], [{**attempt, "status": "failed"}],
                         [{**attempt, "run_id": "another-run"}],
                         [{**attempt, "graph_sha256": "0" * 64}]):
            changed = copy.deepcopy(report)
            changed["verification_test_file_attempts"] = attempts
            self.write_report(changed)
            self.assertNotEqual(self.check().returncode, 0)

    def test_a_discovered_web_file_needs_a_recorded_group_execution(self):
        policy = self.root / "docs/agents/verification-policy.json"
        data = json.loads(policy.read_text())
        data["rules"].insert(0, {"pattern": "apps/web/test/*.test.ts", "kind": "web-test", "group": "node-contract"})
        data["complete"]["groups"]["node-contract"] = ["sample"]
        policy.write_text(json.dumps(data))
        test = self.root / "apps/web/test/new.test.ts"
        test.parent.mkdir(parents=True)
        test.write_text("A newly discovered Web test input.\n")
        self.commit()
        result = self.fixture.cli("run", "--", "make", "verify-local-steps")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No recorded execution covers", self.fixture.report()["error"])

    def test_missing_failed_stale_and_cached_reports_are_refused(self):
        original = json.loads(self.prepare().read_text())
        for change in ({"steps": []}, {"status": "failed"}, {"status": "interrupted"},
                       {"profile": "daily", "cache": {"status": "hit"}}, {"inventory": {"files": []}}, {"environment": {}}, {"rust_test_files": ["missing.rs"]},
                       {"source_end": {**original["source_end"], "inputs_sha256": "0" * 64}}):
            self.write_report({**original, **change})
            self.assertNotEqual(self.check().returncode, 0, change)
        self.body.write_text("PASS: all checks succeeded")
        self.assertNotEqual(self.check().returncode, 0)
        self.body.unlink()
        self.assertNotEqual(self.check().returncode, 0)

    def test_test_lifecycle_requires_new_evidence_and_discovers_new_membership(self):
        self.prepare()
        unsupported = self.root / "scripts/new-test_tests.py"
        unsupported.write_text("raise RuntimeError('must not be silently skipped')\n")
        self.assertNotEqual(self.fixture.cli("inventory", "--check").returncode, 0)
        unsupported.unlink()
        first = self.root / "scripts/first_tests.py"
        renamed = first.with_name("renamed_tests.py")
        for action in (lambda: first.write_text("pass\n"), lambda: first.rename(renamed), renamed.unlink):
            action()
            self.commit()
            self.assertNotEqual(self.check().returncode, 0)
            self.prepare("--policy-reviewed")
            result = self.check()
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_manifest_edits_require_review(self):
        manifest = self.root / "crates/example/Cargo.toml"
        manifest.parent.mkdir(parents=True)
        manifest.write_text('[package]\nname = "example"\n')
        policy = self.root / "docs/agents/verification-policy.json"
        data = json.loads(policy.read_text())
        data["rules"].append({"pattern": "crates/*/Cargo.toml", "kind": "configuration", "group": "complete"})
        policy.write_text(json.dumps(data))
        self.commit()
        self.base = self.fixture.git("rev-parse", "HEAD")
        self.fixture.git("update-ref", "refs/remotes/origin/main", self.base)
        manifest.write_text(manifest.read_text() + 'autotests = false\n')
        self.commit()
        self.prepare()
        self.assertNotEqual(self.check().returncode, 0)

    def test_policy_edits_require_explicit_review_and_base_changes_expire_evidence(self):
        policy = self.root / "docs/agents/verification-policy.json"
        original = policy.read_text()
        data = json.loads(original)
        data["complete"]["stages"].append("previous-stage")
        policy.write_text(json.dumps(data))
        self.commit()
        self.base = self.fixture.git("rev-parse", "HEAD")
        self.fixture.git("update-ref", "refs/remotes/origin/main", self.base)
        policy.write_text(original)
        (self.root / "AGENTS.md").write_text("Changed verification guidance.\n")
        self.commit()
        self.prepare()
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("explicit independent", result.stderr)
        self.prepare("--policy-reviewed")
        self.assertEqual(self.check().returncode, 0)
        self.base = self.fixture.git("rev-parse", "HEAD")
        self.assertNotEqual(self.check().returncode, 0)

    def test_comment_publication_updates_the_candidate_status_and_rejects_the_latest_bad_report(self):
        self.fixture.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-m", "Candidate")
        head = self.fixture.git("rev-parse", "HEAD")
        self.prepare()
        target = self.root / "target"
        (target / "original.txt").write_text(self.body.read_text())
        merge = self.fixture.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit-tree",
                                 "HEAD^{tree}", "-p", self.base, "-p", head, "-m", "Synthetic merge")
        self.fixture.git("update-ref", "refs/pull/1/merge", merge)
        self.fixture.git("remote", "add", "origin", str(self.root))
        self.fixture.git("checkout", "--quiet", "--detach", self.base)
        (target / "pull.json").write_text(json.dumps({"head": {"sha": head}, "base": {"sha": self.base}, "state": "open"}))
        (target / "event.json").write_text(json.dumps({"issue": {"number": 1}}))
        tool = target / "gh"
        tool.write_text(f"#!{sys.executable}\n" + '''import json, pathlib, sys
target = pathlib.Path('target')
path = sys.argv[2]
if path.endswith('/pulls/1'):
    value = json.loads((target / 'pull.json').read_text())
elif '/comments?' in path:
    close = target / 'close_on_comments.json'
    if close.exists():
        (target / 'pull.json').write_text(close.read_text())
        close.unlink()
    value = [[{'id': i, 'user': {'login': 'fixture'}, 'author_association': 'OWNER',
               'body': (target / name).read_text()} for i, name in enumerate(('original.txt', 'evidence.txt'))]]
elif path.endswith('/permission'):
    value = {'permission': 'admin'}
elif '/statuses/' in path:
    with (target / 'statuses.txt').open('a') as log:
        log.write(json.loads(sys.stdin.read())['state'] + '\\n')
    value = {}
else:
    raise SystemExit('Unexpected API request: ' + path)
print(json.dumps(value))
''')
        tool.chmod(0o755)
        self.fixture.environment.update(PATH=str(target) + ":" + self.fixture.environment["PATH"],
                                        GITHUB_EVENT_PATH=str(target / "event.json"), GITHUB_REPOSITORY="fixture/repository", GITHUB_RUN_ID="1")
        result = self.cli("gate")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((target / "statuses.txt").read_text().splitlines(), ["pending", "success"])

        open_pull = {"head": {"sha": head}, "base": {"sha": self.base}, "state": "open"}
        for merged in (False, True):
            closed = {**open_pull, "state": "closed", "merged": merged, "merge_commit_sha": merge if merged else None}
            (target / "pull.json").write_text(json.dumps(closed))
            for event in ({"pull_request": {"number": 1}}, {"issue": {"number": 1}}):
                (target / "event.json").write_text(json.dumps(event))
                result = self.cli("gate")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual((target / "statuses.txt").read_text().splitlines(), ["pending", "success"])

        (target / "pull.json").write_text(json.dumps(open_pull))
        (target / "event.json").write_text(json.dumps({"issue": {"number": 1}}))
        (target / "close_on_comments.json").write_text(json.dumps({**open_pull, "state": "closed", "merged": True, "merge_commit_sha": merge}))
        result = self.cli("gate")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((target / "statuses.txt").read_text().splitlines(), ["pending", "success", "pending"])

        self.body.write_text(self.body.read_text().split("\n", 1)[0] + "\n{}")
        (target / "pull.json").write_text(json.dumps(open_pull))
        (target / "close_on_comments.json").write_text(json.dumps({**open_pull, "state": "closed"}))
        result = self.cli("gate")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((target / "statuses.txt").read_text().splitlines(), ["pending", "success", "pending", "pending"])

        (target / "pull.json").write_text(json.dumps(open_pull))
        self.assertNotEqual(self.cli("gate").returncode, 0)
        self.assertEqual((target / "statuses.txt").read_text().splitlines()[-2:], ["pending", "failure"])
