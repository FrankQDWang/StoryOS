"""Run the Rust positional literal guard through its Make target on a disposable repository."""

import json
import os
from pathlib import Path
import subprocess
import unittest

import verification_tests


MAKEFILE = Path(__file__).resolve().parent.parent / "Makefile"
CALL_SITE = "fn f() { open(true); }\n"


@unittest.skipIf(os.environ.get("STORYOS_PR_BASE_SHA"), "The GitHub verify check has no Rust toolchain")
class LiteralGuardMakeTests(unittest.TestCase):
    def setUp(self):
        self.repo = verification_tests.VerificationCommandTests()
        self.repo.setUp()
        self.addCleanup(self.repo.doCleanups)
        self.root = self.repo.root
        runner = verification_tests.COMMAND.resolve()
        (self.root / "scripts/verification.py").write_text(
            f"import runpy,sys\nsys.path.insert(0, {str(runner.parent)!r})\n"
            f"runpy.run_path({str(runner)!r}, run_name='__main__')\n")
        policy_path = self.root / "docs/agents/verification-policy.json"
        policy = json.loads(policy_path.read_text())
        policy["rules"].append({"pattern": "src/*", "kind": "fixture", "group": "complete"})
        policy_path.write_text(json.dumps(policy))
        (self.root / "docs/agents/rust-literal-exemptions.json").write_text('{"methods": ["enabled"]}\n')
        (self.root / "src").mkdir()
        (self.root / "src/unchanged.rs").write_text(CALL_SITE)
        (self.root / "src/changed.rs").write_text(CALL_SITE)
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Add the Rust fixtures.")

    def test_only_added_lines_of_changed_files_fail_the_make_target(self):
        base = self.repo.git("rev-parse", "HEAD")
        (self.root / "src/changed.rs").write_text(CALL_SITE + "fn g() { open(false); w.enabled(true); }\n")
        (self.root / "src/new.rs").write_text("fn h() { close(/*code*/ 0, None); }\n")
        self.repo.git("add", "src/new.rs")
        result = subprocess.run(["make", "-f", str(MAKEFILE), "rust-literal-guard", f"BASE={base}"],
                                cwd=self.root, env=self.repo.environment, capture_output=True, text=True)
        findings = [line for line in result.stdout.splitlines() if "positional-literal" in line]
        self.assertEqual((result.returncode != 0, findings), (True, [
            "src/changed.rs:2:15: positional-literal: argument `false` of `open` has no /*param*/ comment",
            "src/new.rs:1:28: positional-literal: argument `None` of `close` has no /*param*/ comment",
        ]), result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
