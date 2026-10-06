"""Validate the single-journey command arguments and table with a mocked runner."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import verification_journey


class JourneyCommandTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        journeys = self.root / verification_journey.JOURNEYS
        journeys.mkdir(parents=True)
        (journeys / "s2-statistics.integration.test.ts").write_text("test\n")
        (self.root / ".gitignore").write_text("target/\n")
        for command in (["init", "--quiet"], ["add", "."],
                        ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                         "commit", "--quiet", "-m", "Add a journey."]):
            subprocess.run(["git", *command], cwd=self.root, check=True)
        self.commands = []

    def runner(self, rows, code):
        def run(command, log, root):
            self.commands.append(command)
            log.write_text("mocked\n")
            Path(command[-1], "runs.tsv").write_text("".join("\t".join(row) + "\n" for row in rows))
            return code
        return run

    def journey(self, runner, file="s2-statistics", runs="2", load="0"):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = verification_journey.run(self.root, file, runs, load, runner)
        return code, output.getvalue()

    def test_arguments_are_refused_before_the_runner(self):
        cases = {("../s2-statistics", "1", "0"): "FILE must name one file",
                 ("s2-missing", "1", "0"): "it matches 0",
                 ("s2-statistics", "0", "0"): "RUNS must be an integer from 1 to 100; it is '0'",
                 ("s2-statistics", "x", "0"): "RUNS must be an integer",
                 ("s2-statistics", "1", "-1"): "LOAD must be an integer from 0"}
        for (file, runs, load), message in cases.items():
            with self.subTest(file=file, runs=runs, load=load), self.assertRaisesRegex(ValueError, message):
                verification_journey.run(self.root, file, runs, load, self.runner([], 0))
        (self.root / "dirty").write_text("x\n")
        with self.assertRaisesRegex(ValueError, "A journey run requires a clean"):
            verification_journey.run(self.root, "s2-statistics", "1", "0", self.runner([], 0))
        self.assertEqual(self.commands, [])

    def test_table_reports_pass_count_duration_range_and_load(self):
        code, output = self.journey(self.runner([("1", "passed", "41", "3.10"), ("2", "failed", "47", "4.25")], 1),
                                    file="s2-statistics.integration.test.ts", load="1")
        [record] = (self.root / "target/verification/journeys/s2-statistics").iterdir()
        self.assertEqual(code, 1)
        self.assertEqual(self.commands, [["sh", str(self.root / "scripts/verify-journey.sh"),
                                          "test/browser-exact-dist/s2-statistics.integration.test.ts",
                                          "2", "1", str(record)]])
        self.assertEqual(output, "Journey s2-statistics\n"
                                 "run  status  seconds  load1m\n"
                                 "1    passed  41       3.10\n"
                                 "2    failed  47       4.25\n"
                                 f"Passed: 1 of 2; duration: 41-47 s; records: {record}\n")
        self.assertEqual(json.loads((record / "summary.json").read_text()), {
            "file": "test/browser-exact-dist/s2-statistics.integration.test.ts", "runs": 2, "load": 1,
            "passed": 1, "exit_code": 1,
            "rows": [{"run": 1, "status": "passed", "seconds": 41, "load1m": "3.10"},
                     {"run": 2, "status": "failed", "seconds": 47, "load1m": "4.25"}]})

    def test_a_set_that_stops_early_fails_with_the_missing_runs(self):
        code, output = self.journey(self.runner([], 0), runs="3")
        self.assertEqual(code, 1)
        self.assertIn("Passed: 0 of 3; duration: none;", output)


if __name__ == "__main__":
    unittest.main()
