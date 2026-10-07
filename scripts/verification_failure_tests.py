"""Exercise the flake register and the known flake marker of a failed verification run."""

import json
from pathlib import Path
import sys
import unittest

import verification_failure
import verification_tests

ROOT = Path(__file__).resolve().parent.parent
ENTRY = {"issue": 707, "file": "apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts",
         "test": "repeats Chapter switching", "main": {"passed": 7, "runs": 11, "commit": "15f1eedd",
                                                    "date": "2026-10-05"}, "symptom": "A character was lost."}
RUST = {"issue": 9, "rust": "storyos_server::tests::settles", "main": {"passed": 3, "runs": 4,
                                                                      "commit": "ea69cf3f", "date": "2026-10-06"},
        "symptom": "The settlement timed out."}
TEARDOWN = """error during close AssertionError [ERR_ASSERTION]: Expected values to be strictly deep-equal:
+ actual - expected
+       createVolume: 2,
-       createVolume: 1,
    at /work/apps/web/test/support/exact-dist-global-setup.ts:186:12
    at Object.teardown (/work/apps/web/test/support/required-global-teardown.ts:4:7)
"""


class FlakeRegisterTests(unittest.TestCase):
    def setUp(self):
        self.repo = verification_tests.VerificationCommandTests()
        self.repo.setUp()
        self.addCleanup(self.repo.doCleanups)
        (self.repo.root / verification_failure.REGISTER).write_text(json.dumps([ENTRY, RUST]))
        self.repo.git("add", ".")
        self.repo.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                      "commit", "--quiet", "-m", "Register flakes.")

    def fail_with(self, output):
        script = f"import sys; sys.stdout.write({output!r}); raise SystemExit(1)"
        result = self.repo.cli("run", "--", sys.executable, str(verification_tests.COMMAND), "step", "journeys",
                               "--", sys.executable, "-c", script)
        self.assertEqual(result.returncode, 1, result.stderr)
        return self.repo.report(), result.stdout.splitlines()[-1]

    def test_repository_register_has_the_documented_shape(self):
        entries = json.loads((ROOT / verification_failure.REGISTER).read_text())
        verification_failure.validate(entries)
        self.assertEqual(sorted({entry["issue"] for entry in entries}), [707, 928, 942, 973, 985])
        self.assertEqual([entry["file"] for entry in entries if not (ROOT / entry["file"]).is_file()], [])

    def test_entry_with_a_missing_or_invalid_field_is_refused(self):
        verification_failure.validate([ENTRY, RUST])
        changes = [{key: value for key, value in ENTRY.items() if key != removed} for removed in ENTRY]
        changes += [{**ENTRY, "main": {**ENTRY["main"], **change}} for change in
                    ({"passed": 12}, {"runs": 0}, {"commit": "main"}, {"date": "2026-13-01"}, {"passed": True})]
        changes += [{**ENTRY, "rust": "a::b"}, {**ENTRY, "symptom": "Two\nlines."}, {**ENTRY, "issue": "707"}]
        for entry in changes:
            with self.subTest(entry=entry), self.assertRaises(ValueError):
                verification_failure.validate([entry])

    def test_log_lines_name_vitest_cargo_and_teardown_failures(self):
        log = (" FAIL  |browser-exact-dist (chromium)| test/a.test.ts > group > name\n"
               "   × name 117364ms\n FAIL  |browser-source| test/b.test.ts [ test/b.test.ts ]\n"
               "test storyos_server::tests::settles ... FAILED\ntest other ... ok\n" + TEARDOWN)
        self.assertEqual(verification_failure.failed_tests(log), [
            ("test/a.test.ts", "group > name"), ("test/b.test.ts", ""), ("", "storyos_server::tests::settles"),
            ("/work/apps/web/test/support/exact-dist-global-setup.ts", "global teardown > createVolume")])

    def test_matching_failure_is_marked_as_a_known_flake(self):
        report, line = self.fail_with(" FAIL  |browser-exact-dist (chromium)| "
                                      "test/browser-exact-dist/s2-long-session.integration.test.ts > "
                                      "repeats Chapter switching\ntest storyos_server::tests::settles ... FAILED\n")
        self.assertEqual({key: report[key] for key in ("status", "failed_steps", "known_flake")},
                         {"status": "failed", "failed_steps": ["journeys"], "known_flake": [ENTRY, RUST]})
        self.assertIn("; known flake: #707 (7/11 on main 15f1eedd), #9 (3/4 on main ea69cf3f); report: ", line)

    def test_failure_that_is_not_registered_is_not_marked(self):
        known = " FAIL  |p| test/browser-exact-dist/s2-long-session.integration.test.ts > repeats Chapter switching\n"
        for output in ("compile error\n", " FAIL  |p| test/other.test.ts > repeats Chapter switching\n",
                       known + "test storyos_server::tests::other ... FAILED\n", known + TEARDOWN):
            with self.subTest(output=output):
                for path in self.repo.root.glob("target/verification/*/report.json"):
                    path.unlink()
                report, line = self.fail_with(output)
                self.assertEqual((report["status"], "known_flake" in report), ("failed", False))
                self.assertNotIn("known flake", line)


if __name__ == "__main__":
    unittest.main()
