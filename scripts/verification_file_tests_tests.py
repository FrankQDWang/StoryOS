"""Check whole-file self-test scheduling through the public targeted command."""

import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import unittest

import verification_tests


RUNNER = Path(__file__).with_name("verification_test_files.py")


class FileSelfTests(unittest.TestCase):
    def setUp(self):
        self.fixture = verification_tests.VerificationCommandTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.policy = self.root / "docs/agents/verification-policy.json"
        data = json.loads(self.policy.read_text())
        data["rules"].insert(0, {"pattern": "scripts/*_tests.py", "kind": "verification-test",
                                 "group": "verification-tools"})
        data["complete"] = {"stages": ["verification-tests"],
                            "groups": {"verification-tools": ["verification-tests"]}}
        data["targeted"] = {"verification-tests": {"command": ["python3", "scripts/verification_test_files.py"],
                                                    "clean": False}}
        data["workflow"] = {"version": 1, "operations": {}, "stage_types": {},
                            "profiles": {"verification-tests": {"members": ["verification-tools"]},
                                         "verification-tools": {}},
                            "targeted": {"verification-tests": ["check:verification-tests"]}}
        data["verification_test_workers"] = 2
        data["verification_test_parallel_files"] = []
        self.policy.write_text(json.dumps(data))
        (self.root / "scripts/verification_test_files.py").write_text(
            f"import runpy, sys\nsys.path.insert(0, {str(RUNNER.parent)!r})\n"
            f"runpy.run_path({str(RUNNER)!r}, run_name='__main__')\n")

    def add_file(self, name, *, failure=False, mutate=False):
        path = self.root / "scripts" / f"{name}_tests.py"
        (self.root / "target").mkdir(exist_ok=True)
        os.mkfifo(self.root / "target" / f"release-{name}")
        path.write_text("import json, os, pathlib, time, unittest\n"
                        "class Fixture(unittest.TestCase):\n"
                        "    def test_run(self):\n"
                        "        path = pathlib.Path('target/events.jsonl')\n"
                        "        path.parent.mkdir(exist_ok=True)\n"
                        "        with path.open('a') as output:\n"
                        f"            output.write(json.dumps(['start', '{name}', time.monotonic(), os.environ.get('TMPDIR'), os.getpgrp(), os.environ.get('CARGO_TARGET_DIR')]) + '\\n')\n"
                        f"        print('FILE_READY {name}', flush=True)\n"
                        f"        with open('target/release-{name}') as gate: gate.read()\n"
                        "        with path.open('a') as output:\n"
                        f"            output.write(json.dumps(['end', '{name}', time.monotonic()]) + '\\n')\n"
                        + ("        pathlib.Path('AGENTS.md').write_text('changed source')\n" if mutate else "")
                        + ("        self.fail('fixture failure')\n" if failure else ""))
        return path

    def commit(self, parallel=()):
        data = json.loads(self.policy.read_text())
        data["verification_test_parallel_files"] = [f"scripts/{name}_tests.py" for name in parallel]
        self.policy.write_text(json.dumps(data))
        self.fixture.git("add", ".")
        self.fixture.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                         "commit", "--quiet", "-m", "Declare file self-tests.")

    def start_targeted(self, workers=None):
        env = self.fixture.environment.copy()
        if workers is not None:
            env["STORYOS_VERIFICATION_TEST_WORKERS"] = str(workers)
        env["CARGO_TARGET_DIR"] = str(self.root / "target/foreign-cache")
        env["STORYOS_RUST_CACHE_ROOT"] = str(self.root / "target/foreign-root")
        self.output = b""
        process = subprocess.Popen([sys.executable, str(verification_tests.COMMAND), "targeted", "--check",
                                    "verification-tests"], cwd=self.root, env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
        self.addCleanup(self.stop, process)
        return process

    def stop(self, process):
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
        process.communicate(timeout=20)

    def next_line(self, process):
        while b"\n" not in self.output:
            ready, _, _ = select.select([process.stdout], [], [], 20)
            self.assertTrue(ready, "File test did not reach its coordination point")
            chunk = os.read(process.stdout.fileno(), 4096)
            if not chunk:
                self.fail("File test exited before its coordination point: " + process.stderr.read().decode())
            self.output += chunk
        line, self.output = self.output.split(b"\n", 1)
        return line.decode()

    def ready(self, process, name=None):
        while True:
            line = self.next_line(process)
            if line.startswith("FILE_READY "):
                if name is not None:
                    self.assertEqual(line, f"FILE_READY {name}")
                return line.removeprefix("FILE_READY ")

    def release(self, name):
        with (self.root / "target" / f"release-{name}").open("w") as gate:
            gate.write("go\n")

    def complete(self, process):
        stdout, stderr = process.communicate(timeout=20)
        return subprocess.CompletedProcess(process.args, process.returncode,
                                           (self.output + stdout).decode(), stderr.decode())

    def records(self):
        report_path = next(self.root.glob("target/verification/*/report.json"))
        report = json.loads(report_path.read_text())
        nodes = [json.loads(path.read_text()) for path in (report_path.parent / "nodes").glob("*.json")]
        events = [json.loads(line) for line in (self.root / "target/events.jsonl").read_text().splitlines()]
        return report, nodes, events

    def test_discovery_overlap_cap_serial_barrier_and_node_evidence(self):
        for name in ("a", "aa", "b", "c", "d"):
            self.add_file(name)
        self.commit(parallel=("a", "aa", "b", "d"))
        process = self.start_targeted()
        self.assertEqual({self.ready(process), self.ready(process)}, {"a", "aa"})
        self.release("a")
        self.ready(process, "b")
        self.release("aa")
        self.release("b")
        self.ready(process, "c")
        self.release("c")
        self.ready(process, "d")
        self.release("d")
        result = self.complete(process)
        self.assertEqual(result.returncode, 0, result.stderr)
        report, nodes, events = self.records()
        self.assertEqual(report["status"], "passed")
        self.assertEqual(len(report["steps"]), 1)
        self.assertEqual({node["node_id"] for node in nodes},
                         {f"file:verification-tools:scripts/{name}_tests.py" for name in ("a", "aa", "b", "c", "d")})
        self.assertTrue(all(node["attempt_started"] and node["status"] == "passed" for node in nodes))
        times = {name: {event[0]: event[2] for event in events if event[1] == name}
                 for name in ("a", "aa", "b", "c", "d")}
        self.assertLess(max(times["a"]["start"], times["aa"]["start"]),
                        min(times["a"]["end"], times["aa"]["end"]))
        self.assertLess(times["a"]["end"], times["b"]["start"])
        self.assertLess(max(times[name]["end"] for name in ("a", "aa", "b")), times["c"]["start"])
        self.assertLess(times["c"]["end"], times["d"]["start"])
        self.assertEqual(len({event[3] for event in events if event[0] == "start"}), 5)
        self.assertEqual(len({event[4] for event in events if event[0] == "start"}), 5)
        self.assertTrue(all(event[5] is None for event in events if event[0] == "start"))
        self.assertTrue(all(not Path(event[3]).exists() for event in events if event[0] == "start"))

    def test_serial_diagnostic_failure_and_empty_selection(self):
        self.add_file("a", failure=True)
        self.add_file("b")
        self.commit(parallel=("a", "b"))
        process = self.start_targeted(workers=1)
        self.ready(process, "a")
        self.release("a")
        self.ready(process, "b")
        self.release("b")
        result = self.complete(process)
        self.assertNotEqual(result.returncode, 0)
        report, nodes, events = self.records()
        self.assertEqual(report["status"], "failed")
        self.assertEqual(sorted(node["status"] for node in nodes), ["failed", "passed"])
        self.assertEqual([event[1] for event in events if event[0] == "start"], ["a", "b"])
        self.assertLess(next(event[2] for event in events if event[:2] == ["end", "a"]),
                        next(event[2] for event in events if event[:2] == ["start", "b"]))
        (self.root / "scripts/a_tests.py").unlink()
        (self.root / "scripts/b_tests.py").unlink()
        self.assertNotEqual(subprocess.run([sys.executable, str(verification_tests.COMMAND), "targeted", "--check",
                                           "verification-tests"], cwd=self.root, env=self.fixture.environment,
                                          capture_output=True).returncode, 0)

    def test_source_change_retains_its_status_after_file_failure(self):
        self.add_file("a", failure=True, mutate=True)
        self.commit(parallel=("a",))
        process = self.start_targeted()
        self.ready(process, "a")
        self.release("a")
        self.assertNotEqual(self.complete(process).returncode, 0)
        report, _, _ = self.records()
        self.assertEqual(report["status"], "source-changed")

    @unittest.skipUnless(os.name == "posix", "Process-group interruption requires POSIX.")
    def test_interruption_cleans_file_process_group(self):
        path = self.root / "scripts/a_tests.py"
        path.write_text("import os, signal, unittest\nclass Fixture(unittest.TestCase):\n"
                        "    def test_wait(self):\n"
                        "        print('FILE_READY ' + str(os.getpgrp()), flush=True)\n"
                        "        signal.pause()\n")
        self.commit(parallel=("a",))
        process = subprocess.Popen([sys.executable, str(verification_tests.COMMAND), "targeted", "--check",
                                    "verification-tests"], cwd=self.root, env=self.fixture.environment,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            ready, _, _ = select.select([process.stdout], [], [], 20)
            self.assertTrue(ready)
            line = process.stdout.readline()
            self.assertTrue(line.startswith("FILE_READY "), line)
            group = int(line.split()[1])
            process.send_signal(signal.SIGINT)
            process.communicate(timeout=20)
            self.assertNotEqual(process.returncode, 0)
            with self.assertRaises(ProcessLookupError):
                os.killpg(group, 0)
            report_path = next(self.root.glob("target/verification/*/report.json"))
            self.assertEqual(json.loads(report_path.read_text())["status"], "interrupted")
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


if __name__ == "__main__":
    unittest.main()
