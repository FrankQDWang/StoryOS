"""Exercise file-selected verification through its public command boundary."""

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

import verification_tests


class FilePlanTests(unittest.TestCase):
    def setUp(self):
        self.repo = verification_tests.VerificationCommandTests()
        self.repo.setUp()
        self.addCleanup(self.repo.doCleanups)
        self.root = self.repo.root
        self.policy_path = self.root / "docs/agents/verification-policy.json"
        policy = json.loads(self.policy_path.read_text())
        policy["file_profiles"] = {"node-contract": "// Verification: repository-inputs-only."}
        policy["rules"].insert(0, {"pattern": "apps/web/test/node-contract/*.test.ts",
                                   "kind": "web-test", "group": "node-contract"})
        self.policy_path.write_text(json.dumps(policy))
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Declare test ownership.")
        self.base = self.repo.git("rev-parse", "HEAD")

    def cli(self, action, *args):
        return subprocess.run([sys.executable, str(Path(__file__).with_name("verification_plan.py")),
                               action, "--base", self.base, *args], cwd=self.root,
                              capture_output=True, text=True, env=self.repo.environment)

    def add_test(self, name="new.test.ts"):
        path = self.root / "apps/web/test/node-contract" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("// Verification: repository-inputs-only.\nimport {test} from 'vitest';\ntest('new',()=>{});\n")
        return path

    def test_new_supported_file_is_discovered_without_a_fixed_test_list(self):
        path = self.add_test()
        result = self.cli("plan")
        self.assertEqual(result.returncode, 0, result.stderr)
        plan = json.loads(result.stdout)
        self.assertEqual(plan["changes"], [str(path.relative_to(self.root))])
        self.assertEqual(plan["checks"][-1]["files"], plan["changes"])
        self.assertEqual(plan["checks"][-1]["group"], "node-contract")
        self.assertNotIn("complete", [check["group"] for check in plan["checks"]])

    def test_undeclared_inputs_and_deleted_or_renamed_tests_expand_the_plan(self):
        old = self.add_test("old.test.ts")
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Add the original test.")
        self.base = self.repo.git("rev-parse", "HEAD")
        renamed = old.with_name("renamed.test.ts")
        self.repo.git("mv", str(old), str(renamed))
        extra = self.add_test()
        plan = json.loads(self.cli("plan").stdout)
        self.assertEqual(plan["changes"], sorted(str(p.relative_to(self.root)) for p in (old, renamed, extra)))
        self.assertEqual(plan["checks"][-1]["group"], "complete")
        self.assertNotIn(str(old.relative_to(self.root)), plan["test_files"])
        self.assertIn(str(renamed.relative_to(self.root)), plan["test_files"])
        self.assertTrue(plan["checks"][-1]["reasons"])

    def test_unknown_inputs_and_empty_changes_cannot_report_success(self):
        self.assertNotEqual(self.cli("plan").returncode, 0)
        (self.root / "unknown.xyz").write_text("unowned")
        self.assertNotEqual(self.cli("plan").returncode, 0)

    def test_fixture_or_unreviewed_test_requires_complete_verification(self):
        path = self.add_test()
        path.write_text("import {test} from 'vitest'; test('needs a package',()=>{});\n")
        plan = json.loads(self.cli("plan").stdout)
        self.assertEqual(plan["checks"][-1]["group"], "complete")
        self.assertNotEqual(self.cli("run").returncode, 0)
        self.assertEqual(self.repo.report()["status"], "failed")

    def install_runner_fixture(self):
        policy = json.loads(self.policy_path.read_text())
        policy["rules"].append({"pattern": "Makefile", "kind": "verification", "group": "complete"})
        self.policy_path.write_text(json.dumps(policy))
        (self.root / "Makefile").write_text("verify-policy web-typecheck:\n\t@echo input-policy-checked\n")
        (self.root / ".gitignore").write_text("target/\n.tools/\n")
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Add command fixtures.")
        self.base = self.repo.git("rev-parse", "HEAD")
        tools = self.root / ".tools"
        tools.mkdir()
        pnpm = tools / "pnpm"
        pnpm.write_text(f"#!{sys.executable}\n" + """import json, pathlib, sys
output = next(a.split('=', 1)[1] for a in sys.argv if a.startswith('--outputFile='))
files = [pathlib.Path(a) for a in sys.argv if a.endswith('.test.ts')]
results = [{'name': str(p), 'assertionResults': [] if 'NO_TESTS' in p.read_text()
            else [{'status': 'passed'}]} for p in files]
pathlib.Path(output).write_text(json.dumps({'success': True, 'testResults': results}))
print('executed selected files')
""")
        pnpm.chmod(0o755)
        self.repo.environment = {**self.repo.environment, "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}"}

    def test_daily_dirty_execution_records_plan_and_rejects_empty_discovery(self):
        self.install_runner_fixture()
        path = self.add_test()
        result = self.cli("run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("executed selected files", result.stdout)
        report = self.repo.report()
        self.assertEqual((report["profile"], report["status"]), ("daily", "passed"))
        self.assertTrue(report["source_start"]["dirty"])
        self.assertEqual(report["plan"]["source"], report["source_start"])
        self.assertTrue(report["plan"]["digest"])
        path.write_text(path.read_text() + "// NO_TESTS\n")
        self.assertNotEqual(self.cli("run").returncode, 0)

    def test_saved_plan_rejects_changed_bytes_even_when_the_tree_stays_dirty(self):
        path = self.add_test()
        plan = self.cli("plan")
        target = self.root / "target"
        target.mkdir()
        saved = target / "plan.json"
        saved.write_text(plan.stdout)
        path.write_text(path.read_text() + "// changed input\n")
        result = self.cli("run", "--plan", str(saved))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("stale", result.stderr.lower())

    def test_staged_changes_remain_selected_when_working_bytes_match_the_base(self):
        path = self.add_test()
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Add the base test.")
        self.base = self.repo.git("rev-parse", "HEAD")
        original = path.read_text()
        path.write_text(original + "// staged\n")
        self.repo.git("add", ".")
        path.write_text(original)
        plan = json.loads(self.cli("plan").stdout)
        self.assertEqual(plan["changes"], [str(path.relative_to(self.root))])

    def install_cargo_fixture(self):
        policy = json.loads(self.policy_path.read_text())
        policy["rules"][:0] = [
            {"pattern": "crates/*/src/*", "kind": "rust-source", "group": "cargo"},
            {"pattern": "*.toml", "kind": "configuration", "group": "complete"},
            {"pattern": "Cargo.lock", "kind": "configuration", "group": "complete"},
        ]
        self.policy_path.write_text(json.dumps(policy))
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/core", "crates/api"]\nresolver="2"\n')
        for name in ("core", "api"):
            folder = self.root / "crates" / name
            (folder / "src").mkdir(parents=True)
            (folder / "src/lib.rs").write_text("pub fn value() -> u8 { 1 }\n")
            dependency = '\n[dependencies]\ncore={path="../core"}\n' if name == "api" else ""
            (folder / "Cargo.toml").write_text(f'[package]\nname="{name}"\nversion="0.1.0"\nedition="2021"\n{dependency}')
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root, check=True, capture_output=True)
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Add local Cargo targets.")
        self.base = self.repo.git("rev-parse", "HEAD")

    def test_rust_source_lists_current_targets_and_reverse_dependencies(self):
        self.install_cargo_fixture()
        (self.root / "crates/core/src/lib.rs").write_text("pub fn value() -> u8 { 2 }\n")
        result = self.cli("plan")
        self.assertEqual(result.returncode, 0, result.stderr)
        plan = json.loads(result.stdout)
        self.assertEqual(sorted(plan["cargo_targets"]), ["api", "core"])
        self.assertEqual(plan["checks"][-1]["group"], "complete")

    def test_rust_file_runs_its_crate_and_refuses_zero_discovered_tests(self):
        self.install_cargo_fixture()
        policy = json.loads(self.policy_path.read_text())
        policy["file_profiles"]["cargo"] = "// Verification: repository-inputs-only."
        policy["rules"].insert(0, {"pattern": "crates/*/src/*_tests.rs", "kind": "rust-test", "group": "cargo"})
        self.policy_path.write_text(json.dumps(policy))
        (self.root / "crates/core/src/lib.rs").write_text('#[cfg(test)]\n#[path="value_tests.rs"]\nmod tests;\n')
        path = self.root / "crates/core/src/value_tests.rs"
        marker = "// Verification: repository-inputs-only.\n"
        path.write_text(marker + "#[test]\nfn boundary() { assert!(Some(1).is_some()); }\n")
        self.install_runner_fixture()
        path.write_text(path.read_text() + "// changed test input\n")
        result = self.cli("run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.repo.report()["plan"]["checks"][-1]["group"], "cargo:core")
        complete = self.repo.cli("rust-tests")
        self.assertEqual(complete.returncode, 0, complete.stderr)
        orphan = path.with_name("orphan_tests.rs")
        orphan.write_text(marker + "this is not valid Rust\n")
        result = self.cli("run")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not compiled into a test target", result.stderr)
        self.assertIn("not compiled into a test target", self.repo.cli("rust-tests").stderr)
        orphan.unlink()
        for attribute in ("#[ ignore ]", "#[cfg_attr(test, ignore)]"):
            path.write_text(marker + attribute + "\n#[test]\nfn excluded() { panic!(); }\n")
            self.assertEqual(json.loads(self.cli("plan").stdout)["checks"][-1]["group"], "complete")
        path.write_text(marker)
        self.assertNotEqual(self.cli("run").returncode, 0)
