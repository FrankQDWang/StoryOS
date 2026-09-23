"""Exercise shared test discovery and execution with observable child commands."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import unittest

import verification_tests


class SharedCommandTests(unittest.TestCase):
    def setUp(self):
        self.repo = verification_tests.VerificationCommandTests()
        self.repo.setUp()
        self.addCleanup(self.repo.doCleanups)
        self.root = self.repo.root
        self.policy = self.root / "docs/agents/verification-policy.json"
        policy = json.loads(self.policy.read_text())
        policy["rules"][:0] = [
            {"pattern": "apps/web/test/node-postgresql/*.test.ts", "kind": "web-test", "group": "node-postgresql"},
            {"pattern": "crates/*", "kind": "fixture", "group": "complete"},
        ]
        policy["shared_phases"] = [
            {"name": "first", "group": "node-postgresql", "stage": "http-files", "prepare": "none"},
            {"name": "second", "group": "node-postgresql", "stage": "http-files", "prepare": "reset-challenge"},
            {"name": "third", "group": "node-postgresql", "stage": "http-files", "prepare": "reload-fixture"},
        ]
        self.policy.write_text(json.dumps(policy))
        self.a = self.add_test("z", "first")
        self.b = self.add_test("a", "first", [self.a])
        self.c = self.add_test("c", "second")
        self.d = self.add_test("d", "third")
        tools = self.root / "target/tools"
        tools.mkdir(parents=True)
        for name in ("docker", "pnpm"):
            executable = tools / name
            executable.write_text(f"#!{sys.executable}\n" + """import json, os, pathlib, sys
with pathlib.Path('target/children.jsonl').open('a') as output:
    output.write(json.dumps([pathlib.Path(sys.argv[0]).name, sys.argv[1:],
                             os.environ.get('STORYOS_VITEST_FILE_ORDER')]) + '\\n')
for arg in sys.argv:
    if arg.startswith('--outputFile='):
        files = [str((pathlib.Path('apps/web') / p).resolve()) for p in sys.argv if p.endswith('.test.ts')]
        pathlib.Path(arg.split('=', 1)[1]).write_text(json.dumps({'success': True, 'testResults': [
            {'name': p, 'assertionResults': [{'status': 'passed'}]} for p in files]}))
""")
            executable.chmod(0o755)
        self.repo.environment.update(PATH=f"{tools}{os.pathsep}{os.environ['PATH']}",
                                     STORYOS_TEST_POSTGRES_CONTAINER="fixture")
        (self.root / "scripts/lib").mkdir()
        shutil.copy(Path(__file__).parent / "lib/controlled-postgres.sh", self.root / "scripts/lib")
        fixture = self.root / "crates/storyos-adapter-postgres/tests/fixture.sql"
        fixture.parent.mkdir(parents=True)
        fixture.write_text("SELECT 1;\n")
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Register shared test inputs.")

    def add_test(self, name, phase, after=()):
        path = f"apps/web/test/node-postgresql/{name}.test.ts"
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text('// Verification: ' + json.dumps({"phase": phase, "after": list(after)}) + '\n')
        return path

    def cli(self, action, *args):
        return subprocess.run([sys.executable, str(Path(__file__).with_name("verification_shared.py")), action, *args],
                              cwd=self.root, env=self.repo.environment, capture_output=True, text=True)

    def test_discovered_order_and_fixture_boundaries_drive_real_children(self):
        extra = self.add_test("b", "second", [self.c])
        result = self.cli("plan")
        self.assertEqual(result.returncode, 0, result.stderr)
        phases = json.loads(result.stdout)
        self.assertEqual([p["files"] for p in phases], [[self.a, self.b], [self.c, extra], [self.d]])
        result = self.cli("run")
        self.assertEqual(result.returncode, 0, result.stderr)
        children = [json.loads(line) for line in (self.root / "target/children.jsonl").read_text().splitlines()]
        self.assertEqual([c[0] for c in children], ["pnpm", "docker", "pnpm", "docker", "docker", "pnpm"])
        self.assertIn("UPDATE storyos.project_command_challenge_rate_windows", " ".join(children[1][1]))
        self.assertIn("TRUNCATE", " ".join(children[3][1]))
        for phase, child in zip(phases, [c for c in children if c[0] == "pnpm"]):
            files = [path.removeprefix("apps/web/") for path in phase["files"]]
            self.assertEqual(child[1], ["--dir", "apps/web", "exec", "vitest", "run", "--project", "node-postgresql", *files])
            self.assertEqual(child[2], ":".join(files))

    def test_rename_and_delete_refresh_membership_and_reject_dangling_edges(self):
        renamed = self.a.replace("z.test", "renamed.test")
        (self.root / self.a).rename(self.root / renamed)
        self.assertNotEqual(self.cli("plan").returncode, 0)
        self.add_test("a", "first", [renamed])
        self.assertEqual(json.loads(self.cli("plan").stdout)[0]["files"], [renamed, self.b])
        (self.root / self.b).unlink()
        self.assertEqual(json.loads(self.cli("plan").stdout)[0]["files"], [renamed])
        (self.root / renamed).unlink()
        self.assertIn("empty", self.cli("plan").stderr)

    def test_daily_database_selection_preserves_resets_without_other_groups(self):
        result = self.cli("run", "--groups", "database")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "target/children.jsonl").exists())
        result = self.cli("run", "--groups", "node-postgresql")
        self.assertEqual(result.returncode, 0, result.stderr)
        children = [json.loads(line) for line in (self.root / "target/children.jsonl").read_text().splitlines()]
        self.assertEqual([c[0] for c in children], ["pnpm", "docker", "pnpm", "docker", "docker", "pnpm"])

    def test_daily_database_entry_prepares_once_and_keeps_workspace_features(self):
        policy = json.loads(self.policy.read_text())
        stages = ['postgres-scope', 'postgres-challenge', 'postgres-library', 'http-files']
        policy['complete'] = {'stages': stages, 'groups': {'node-postgresql': ['http-files']}}
        policy['workflow'] = {'version': 1, 'operations': {}, 'targeted': {}, 'stage_types': {},
                              'profiles': {'daily-database': {}, 'node-postgresql': {}}}
        self.policy.write_text(json.dumps(policy))
        scripts = Path(__file__).parent
        for path in scripts.glob("verification*.py"):
            if not path.name.endswith("_tests.py"):
                shutil.copy(path, self.root / "scripts")
        shutil.copy(scripts / "verify-daily-database.sh", self.root / "scripts")
        tools = self.root / "target/tools"
        docker = tools / "docker"
        docker.write_text(docker.read_text() + "\nprint('PostgreSQL init process complete' if 'logs' in sys.argv else '127.0.0.1:5432')\n")
        shutil.copy(tools / "pnpm", tools / "cargo")
        package = self.root / "target/release-package"
        package.mkdir()
        shutil.copy(tools / "pnpm", package / "storyos-storage")
        result = subprocess.run(["sh", "verify-daily-database.sh", "database", "node-postgresql"],
                                cwd=self.root / "scripts", env=self.repo.environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        children = [json.loads(line) for line in (self.root / "target/children.jsonl").read_text().splitlines()]
        self.assertEqual(sum(c[0] == "docker" and c[1][0] == "run" for c in children), 1)
        self.assertEqual(sum(c[0] == "storyos-storage" for c in children), 1)
        cargo = [c[1] for c in children if c[0] == "cargo"]
        self.assertEqual(cargo, [["test", "--locked", "--workspace", "--all-features", *target,
                                 "--", "--ignored", "--nocapture"] for target in
                                [["--test", "project_scope"], ["--test", "project_command_challenge"], ["--lib"]]])
        self.assertFalse(any("browser-exact-dist" in c[1] or "verify-local-steps" in c[1] for c in children))
        report = self.repo.report()
        self.assertTrue(all(step.get('node_id') for step in report['steps']))
        self.assertEqual({step['node_id'] for step in report['steps']},
                         {'check:daily-database', 'check:postgres-scope', 'check:postgres-challenge',
                          'check:postgres-library', 'phase:first', 'phase:second', 'phase:third'})

    def test_invalid_declarations_fail_policy_and_execution_before_children(self):
        original = (self.root / self.a).read_text()
        cases = ["", original * 2,
                 '// Verification: {"phase":"unknown","after":[]}\n',
                 '// Verification: {"phase":"first","after":["missing"]}\n',
                 '// Verification: ' + json.dumps({"phase": "first", "after": [self.c]}) + '\n',
                 '// Verification: ' + json.dumps({"phase": "first", "after": [self.b]}) + '\n']
        for content in cases:
            with self.subTest(content=content):
                (self.root / self.a).write_text(content)
                self.assertNotEqual(self.repo.cli("inventory", "--check").returncode, 0)
                self.assertNotEqual(self.cli("run").returncode, 0)
                self.assertFalse((self.root / "target/children.jsonl").exists())
        (self.root / self.a).write_text(original)
        policy = json.loads(self.policy.read_text())
        policy["shared_phases"].append(policy["shared_phases"][0])
        self.policy.write_text(json.dumps(policy))
        self.assertNotEqual(self.repo.cli("inventory", "--check").returncode, 0)

    def test_package_prerequisite_rejects_bad_membership_before_node_install(self):
        for name in ("verification.py", "verification_shared.py", "verification_cache.py", "verification_daily.py",
                     "verification_status.py", "verification_graph.py", "verification_candidate.py",
                     "verification_rust_cache.py", "verification_legacy_cache.py"):
            shutil.copy(Path(__file__).parent / name, self.root / "scripts")
        (self.root / self.a).write_text("// Missing declaration.\n")
        result = subprocess.run(["make", "-f", str(Path(__file__).resolve().parent.parent / "Makefile"),
                                 "web-typecheck"], cwd=self.root, env=self.repo.environment,
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("require one first-line shared phase declaration", result.stderr)
        self.assertFalse((self.root / "target/children.jsonl").exists())
