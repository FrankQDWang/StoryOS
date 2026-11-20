#!/usr/bin/env python3
"""Tests of scripts/check-lanes.py (issues 1711, 1712) and scripts/browser-vitest.sh (issue 1734).

CHECK_LANES_SCRIPT gives another runner to test, for example the runner of an earlier commit.
"""

import contextlib
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
RUNNER = os.environ.get("CHECK_LANES_SCRIPT", str(ROOT / "scripts/check-lanes.py"))
# scripts/dev-postgres.sh names each container of a `with` run storyos-check-<checkout id>-<pid>.
CHECKOUT_ID = hashlib.sha256(os.path.realpath(ROOT).encode()).hexdigest()[:20]
IMAGE_LINE = re.search(r"^storyos_postgres_image=(\S+)$",
                       (ROOT / "scripts/lib/controlled-postgres.sh").read_text(), re.MULTILINE)
if IMAGE_LINE is None:
    raise SystemExit("scripts/lib/controlled-postgres.sh does not set storyos_postgres_image")
IMAGE = IMAGE_LINE.group(1)


def runner(*lanes, **options):
    arguments = [sys.executable, RUNNER]
    for lane in lanes:
        arguments += ["--lane", lane]
    return subprocess.Popen(arguments, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, **options)


def wait_for(path):
    """A shell loop that waits at most 60 s for the file."""
    return f"i=0; while [ ! -e {path} ] && [ $i -lt 600 ]; do sleep 0.1; i=$((i + 1)); done"


def docker_names(command, field="{{.Name}}"):
    return subprocess.run(["docker", *command, "--format", field], capture_output=True, text=True,
                          check=True).stdout.split()


def containers():
    return docker_names(["ps", "-a"], "{{.Names}}")


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def pids_of(command_line):
    """Return the IDs of the processes whose command line is exactly this text."""
    return [int(line.split(None, 1)[0]) for line in subprocess.run(
        ["ps", "-A", "-o", "pid=,command="], capture_output=True, text=True, check=True).stdout.splitlines()
        if line.split(None, 1)[1:] == [command_line]]


class StopTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="storyos-check-lanes-"))
        self.marker = uuid.uuid4().hex[:12]
        # A unique duration identifies the sleep of a test, for example "sleep 287.004211".
        self.sleep = f"sleep 287.{uuid.uuid4().int % 1000000:06d}"
        self.watched = []

    def tearDown(self):
        for pid in self.watched + pids_of(self.sleep):
            if alive(pid):
                os.kill(pid, signal.SIGKILL)

    def wait_for_sleep(self):
        deadline = time.monotonic() + 60
        while not pids_of(self.sleep) and time.monotonic() < deadline:
            time.sleep(0.1)
        self.watched += pids_of(self.sleep)
        self.assertTrue(self.watched, f"{self.sleep} did not start")

    def assert_gone(self, pid, output):
        deadline = time.monotonic() + 5
        while alive(pid) and time.monotonic() < deadline:
            time.sleep(0.1)
        self.assertFalse(alive(pid), f"the process {pid} of the run still runs\n{output}")

    def interrupt(self, process):
        os.killpg(process.pid, signal.SIGINT)
        output, _ = process.communicate(timeout=120)
        self.assertEqual(process.returncode, 130, output)
        self.assertIn("check-lanes: interrupted. The lanes are stopped.", output)
        return output

    def test_the_stop_removes_only_the_docker_resources_of_its_own_run(self):
        own, unlabelled, other_run = (f"storyos-check-{CHECKOUT_ID}-{kind}-{self.marker}"
                                      for kind in ("own", "unlabelled", "other-run"))
        label = "--label \"storyos.check-run=$STORYOS_CHECK_RUN\""
        try:
            subprocess.run(["docker", "create", "--name", unlabelled, IMAGE], capture_output=True, check=True)
            subprocess.run(["docker", "create", "--name", other_run, "--label", "storyos.check-run=another-run",
                            IMAGE], capture_output=True, check=True)
            created = self.directory / "created"
            # The container uses the network and the volume, as a container of the recovery drill.
            process = runner(
                f"lanes-test-container=docker network create {label} {own} >/dev/null; "
                f"docker volume create {label} {own} >/dev/null; "
                f"docker create --name {own} {label} --network {own} --volume {own}:/data {IMAGE} >/dev/null; "
                f"touch {created}; {self.sleep}",
                f"lanes-test-container-fails={wait_for(created)}; exit 1")
            output, _ = process.communicate(timeout=120)
            self.assertEqual(process.returncode, 1, output)
            remaining = containers()
            self.assertNotIn(own, remaining, output)
            self.assertNotIn(own, docker_names(["network", "ls"]), output)
            self.assertNotIn(own, docker_names(["volume", "ls"]), output)
            self.assertIn(unlabelled, remaining, output)
            self.assertIn(other_run, remaining, output)
            self.assertEqual(pids_of(self.sleep), [], output)
        finally:
            subprocess.run(["docker", "rm", "-fv", own, unlabelled, other_run], capture_output=True)
            subprocess.run(["docker", "network", "rm", own], capture_output=True)
            subprocess.run(["docker", "volume", "rm", "-f", own], capture_output=True)

    def test_an_interruption_stops_a_lane_of_only_sleep(self):
        process = runner(f"lanes-test-sleep={self.sleep}", start_new_session=True)
        self.wait_for_sleep()
        output = self.interrupt(process)
        for pid in self.watched:
            self.assert_gone(pid, output)

    def test_a_failed_lane_stops_a_lane_of_only_sleep(self):
        started = self.directory / "started"
        process = runner(f"lanes-test-sleep=touch {started}; {self.sleep}",
                         f"lanes-test-sleep-fails={wait_for(started)}; sleep 1; exit 1")
        self.wait_for_sleep()
        output, _ = process.communicate(timeout=120)
        self.assertEqual(process.returncode, 1, output)
        for pid in self.watched:
            self.assert_gone(pid, output)

    def test_the_stop_reaches_a_child_in_a_new_session_of_a_running_lane(self):
        # The child has no environment, as Google Chrome, whose environment ps does not show.
        child_pid = self.directory / "child-pid"
        spawn = ("import subprocess, sys, time; child = subprocess.Popen([sys.executable, '-c', "
                 "'import time; time.sleep(300)'], start_new_session=True, env={}); "
                 f"open('{child_pid}', 'w').write(str(child.pid)); time.sleep(300)")
        process = runner(f"lanes-test-new-session={sys.executable} -c \"{spawn}\"",
                         f"lanes-test-new-session-fails={wait_for(child_pid)}; exit 1")
        output, _ = process.communicate(timeout=120)
        pid = int(child_pid.read_text())
        self.watched.append(pid)
        self.assertEqual(process.returncode, 1, output)
        self.assert_gone(pid, output)

    def test_an_interruption_during_the_stop_does_not_end_the_stop(self):
        lane_pid = self.directory / "lane-pid"
        # The trap keeps the lane for 2 s after SIGTERM, so the runner scans the processes during that time.
        process = runner(f"lanes-test-slow=trap 'sleep 2; exit 143' TERM; echo $$ > {lane_pid}; {self.sleep} & wait",
                         start_new_session=True)
        self.wait_for_sleep()
        deadline = time.monotonic() + 60
        while not lane_pid.exists() and time.monotonic() < deadline:
            time.sleep(0.1)
        self.watched.append(int(lane_pid.read_text()))
        # Ctrl-C in a terminal sends SIGINT to each process of the foreground process group.
        end = time.monotonic() + 3
        while time.monotonic() < end and process.poll() is None:
            os.killpg(process.pid, signal.SIGINT)
            time.sleep(0.01)
        output, _ = process.communicate(timeout=120)
        self.assertEqual(process.returncode, 130, output)
        self.assertIn("check-lanes: interrupted. The lanes are stopped.", output)
        self.assertNotIn("Traceback", output)
        for pid in self.watched:
            self.assert_gone(pid, output)


class BrowserStartTests(unittest.TestCase):
    def test_two_starts_of_chrome_have_the_gap(self):
        # The gap of scripts/check-lanes.py BROWSER_START_GAP_SECONDS.
        gap = float(re.search(r"^BROWSER_START_GAP_SECONDS = (\d+)$", Path(RUNNER).read_text(), re.MULTILINE).group(1))
        # Its own record, so that the test does not change the gap of the browser lanes of a run.
        record = Path(tempfile.mkdtemp(prefix="storyos-browser-start-")) / "browser-start"
        starts = []
        for _ in range(2):
            subprocess.run([sys.executable, RUNNER, "--browser-start"], cwd=ROOT, check=True, timeout=60,
                           env={**os.environ, "STORYOS_BROWSER_START_FILE": str(record)})
            starts.append(float(record.read_text()))
        self.assertGreaterEqual(starts[1] - starts[0], gap)

    def test_a_claim_without_wait_does_not_wait_for_a_held_record(self):
        module = runner_module()
        # A root of its own, so that the test does not change the gap of the browser lanes of a run.
        root = Path(tempfile.mkdtemp(prefix="storyos-browser-start-"))
        record = root / module.BROWSER_START_FILE
        record.parent.mkdir(parents=True)
        with open(record, "a+") as holder:
            fcntl.flock(holder, fcntl.LOCK_EX)
            began = time.monotonic()
            self.assertFalse(module.claim_browser_start(root, wait=False))
            self.assertLess(time.monotonic() - began, 5)


def runner_module():
    """Load the runner as a module, for the tests of its functions."""
    spec = importlib.util.spec_from_file_location("check_lanes", RUNNER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_lanes(module, lanes, root=ROOT, final_checks=()):
    """Run the lanes in this process. Return the exit code, the output, and the time of the run."""
    handlers = {number: signal.getsignal(number) for number in module.STOP_SIGNALS}
    output, began = io.StringIO(), time.monotonic()
    try:
        with contextlib.redirect_stdout(output):
            code = module.run(root, lanes, list(final_checks))
    finally:
        for number, handler in handlers.items():
            signal.signal(number, handler)
    return code, output.getvalue(), time.monotonic() - began


class FunctionTests(unittest.TestCase):
    def test_a_screenshot_directory_is_not_a_test_file(self):
        web = Path(tempfile.mkdtemp(prefix="storyos-test-files-"))
        (web / "test/journeys/__screenshots__/a.integration.test.ts").mkdir(parents=True)
        (web / "test/journeys/a.integration.test.ts").write_text("")
        self.assertEqual(runner_module().test_files(web, "test/journeys"), ["test/journeys/a.integration.test.ts"])

    def test_the_project_scope_lanes_wait_for_the_build_lane(self):
        # The PostgreSQL shards come from the build lane. When the release package is up to date, a project-scope
        # lane that waits only for the package lane takes a lane before them.
        lanes = runner_module().project_scope_lanes(ROOT)
        self.assertTrue(lanes)
        for lane in lanes:
            self.assertEqual(sorted(lane.after), ["build", "package"], lane.name)

    def test_at_most_two_browser_lanes_run_at_the_same_time(self):
        module = runner_module()
        lanes = [module.Lane(f"lanes-test-browser-{index}", ["sleep", "2"], browser=True, browser_at_start=False)
                 for index in range(3)]
        lanes.append(module.Lane("lanes-test-no-browser", ["sleep", "2"]))
        handlers = {number: signal.getsignal(number) for number in module.STOP_SIGNALS}
        try:
            self.assertEqual(module.run(ROOT, lanes, []), 0)
        finally:
            for number, handler in handlers.items():
                signal.signal(number, handler)
        browsers = lanes[:3]
        for lane in browsers:
            running = [other for other in browsers
                       if other.started <= lane.started < other.started + other.duration]
            self.assertLessEqual(len(running), 2, lane.name)
        # The lane without a browser starts while the third browser lane waits.
        self.assertLess(lanes[3].started, lanes[2].started)

    def test_a_lane_that_does_not_end_in_the_time_limit_fails_the_run(self):
        # The lane and its child in a new session do not obey SIGTERM, as the Vitest run of issue 1728. Each writes
        # its ID after it ignores SIGTERM. Without a time limit, the lane passes after 20 s.
        module, directory = runner_module(), Path(tempfile.mkdtemp(prefix="storyos-lane-limit-"))
        module.LANE_TIME_LIMIT_SECONDS, module.STOP_SECONDS = 15, 2
        ready = ("import os, signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                 "open({!r}, 'w').write(str(os.getpid())); ")
        child = [sys.executable, "-c", ready.format(str(directory / "child")) + "time.sleep(20)"]
        lane = module.Lane("lanes-test-no-end", [sys.executable, "-c", (
            ready.format(str(directory / "lane"))
            + f"import subprocess; subprocess.Popen({child!r}, start_new_session=True); time.sleep(20)")])
        try:
            code, text, elapsed = run_lanes(module, [lane])
            watched = [int((directory / name).read_text()) for name in ("lane", "child")
                       if (directory / name).exists()]
            # The system reaps a killed child of another parent a short time after its end.
            deadline = time.monotonic() + 5
            while any(alive(pid) for pid in watched) and time.monotonic() < deadline:
                time.sleep(0.1)
            remaining = [pid for pid in watched if alive(pid)]
        finally:
            for name in ("lane", "child"):
                if (directory / name).exists() and alive(int((directory / name).read_text())):
                    os.kill(int((directory / name).read_text()), signal.SIGKILL)
        self.assertEqual(code, 1, text)
        self.assertRegex(text, r"Lane lanes-test-no-end: FAIL after \d+ s \(the lane ran longer than the time limit "
                               r"of 15 s\)")
        # The limit, then SIGTERM and SIGKILL of the lane, then SIGTERM and SIGKILL of the child, then Docker.
        self.assertLess(elapsed, 15 + 2 * (module.STOP_SECONDS + module.KILL_SECONDS) + 15, text)
        self.assertEqual(len(watched), 2, f"the lane or its child did not start before the stop\n{text}")
        self.assertEqual(remaining, [], text)

    def test_a_stop_whose_scan_of_the_processes_fails_still_ends_the_lanes(self):
        # A ps command that does not end, and a ps line that does not parse.
        for error in (subprocess.TimeoutExpired(["ps"], 60), ValueError("not enough values to unpack")):
            with self.subTest(error=type(error).__name__):
                module, removals = runner_module(), []

                def process_table(error=error):
                    raise error

                module.process_table, module.remove_run_resources = process_table, removals.append
                # The lane does not obey SIGTERM, so only the SIGKILL ends it. It writes a line after it ignores it.
                lane = module.Lane("lanes-test-scan-fails", [])
                lane.process = subprocess.Popen([sys.executable, "-c", (
                    "import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); print('ready', flush=True); "
                    "time.sleep(287)")], start_new_session=True, stdout=subprocess.PIPE, text=True)
                self.assertEqual(lane.process.stdout.readline(), "ready\n")
                handlers = {number: signal.getsignal(number) for number in module.STOP_SIGNALS}
                try:
                    with contextlib.redirect_stdout(io.StringIO()):
                        self.assertFalse(module.stop([lane], "token"))
                    self.assertEqual(lane.process.wait(timeout=30), -signal.SIGKILL)
                    self.assertEqual(removals, ["token"])
                finally:
                    for number, handler in handlers.items():
                        signal.signal(number, handler)
                    if lane.process.poll() is None:
                        os.killpg(lane.process.pid, signal.SIGKILL)
                        lane.process.wait()
                    lane.process.stdout.close()

    def test_a_command_of_the_runner_that_does_not_end_fails_in_its_time_limit(self):
        module = runner_module()
        module.TOOL_SECONDS = 1
        began = time.monotonic()
        with self.assertRaises(subprocess.TimeoutExpired):
            module.tool_output(["sleep", "30"])
        self.assertLess(time.monotonic() - began, 10)

        def tool_output(command):
            raise subprocess.TimeoutExpired(command, module.TOOL_SECONDS)

        # A Docker command that does not end makes the removal not verified, and the stop continues to its result.
        module.tool_output = tool_output
        self.assertFalse(module.remove_run_resources("token"))

    def test_a_report_of_a_run_that_did_not_connect_to_its_browser_fails_the_lane(self):
        # The shape of the second report of the lane journeys-2 in a run of `make check-full` on 2026-11-20 (issue
        # 1728): Vitest did not connect to its browser session, and its report lists each file as passed with no test.
        module, root = runner_module(), Path(tempfile.mkdtemp(prefix="storyos-lane-report-"))
        web, reports = root / "apps/web", root / "target/check"
        reports.mkdir(parents=True)
        first = "test/browser-exact-dist/s2-workspace.integration.test.ts"
        files = [f"test/browser-exact-dist/journey-{index}.integration.test.ts" for index in range(15)]

        def report(suites):
            return json.dumps({"numTotalTests": sum(tests for _, tests in suites), "success": True,
                               "testResults": [{"name": str(web / file), "status": "passed",
                                                "assertionResults": [{"status": "passed"}] * tests}
                                               for file, tests in suites]})

        cases = (("journeys-test", lambda: module.journey_lane(root, "journeys-test", [first, *files]),
                  {"journeys-test-1.json": [(first, 1)], "journeys-test-2.json": [(file, 0) for file in files]}),
                 ("vitest-test", lambda: module.vitest_lane(root, "vitest-test", "browser-exact-dist", files),
                  {"vitest-test.json": [(file, 0) for file in files]}))
        for name, lane, outputs in cases:
            for output, suites in outputs.items():
                (reports / output).write_text(report(suites))
            with self.assertRaisesRegex(ValueError, "has no passing test", msg=name):
                lane().passed("")
            # The same report with one passing test in each file passes, so the check refuses only the missing tests.
            for output, suites in outputs.items():
                (reports / output).write_text(report([(file, 1) for file, _ in suites]))
            self.assertEqual(lane().passed(""), [], name)

    def test_a_failed_container_removal_does_not_stop_the_removal_of_networks_and_volumes(self):
        module, removed = runner_module(), []

        def tool_output(command):
            if command[:2] == ["docker", "rm"]:
                raise subprocess.CalledProcessError(1, command)
            if "ls" in command or "ps" in command:
                return f"{command[1]}-of-the-run\n"
            removed.append(command[1])
            return ""

        module.tool_output = tool_output
        self.assertFalse(module.remove_run_resources("token"))
        self.assertEqual(removed, ["network", "volume"])


# The output of Vitest 4.1.11 for a run that connected to no browser session (issue 1728).
NO_BROWSER = ('Error: Failed to connect to the browser session "b63626c2" [browser-exact-dist (chromium)] within the '
              "timeout.\n\n Test Files   (1)\n      Tests  no tests\n     Errors  1 error\n")
PASSED = " Test Files  1 passed (1)\n      Tests  1 passed (1)\n"
FAILED_TEST = " Test Files  1 failed (1)\n      Tests  1 failed (1)\n"
# A test ran, and its own text has the words of the summary of a run with no test.
CONNECT_LINE_WITH_TESTS = ('Error: Failed to connect to the browser session "b63626c2" [browser-exact-dist (chromium)] '
                           "within the timeout.\nstdout | a.integration.test.ts > the Tests panel shows no tests\n"
                           " Test Files  1 failed (1)\n      Tests  1 failed | 2 passed (3)\n")


def fake_browser_vitest_root(answers, failed_gaps=(), failed_tee=False):
    """A root with scripts/browser-vitest.sh, a fake start gap, and a fake `pnpm` that plays one answer for each run.

    `pnpm exec` takes node_modules/.bin before the PATH, so the fake is `pnpm`. Each wait and run goes to <root>/record.
    The waits with a number in failed_gaps fail. With failed_tee, a fake `tee` reads its input and fails.
    """
    root = Path(tempfile.mkdtemp(prefix="storyos-browser-vitest-"))
    for directory in ("scripts", "bin", "target/check"):
        (root / directory).mkdir(parents=True)
    shutil.copy(ROOT / "scripts/browser-vitest.sh", root / "scripts")
    (root / "answers.json").write_text(json.dumps(answers))
    (root / "scripts/check-lanes.py").write_text(
        "import sys\n"
        "from pathlib import Path\n"
        "path = Path(__file__).resolve().parent.parent / 'record'\n"
        "number = (path.read_text().splitlines() if path.exists() else []).count('gap') + 1\n"
        f"failed = number in {sorted(failed_gaps)!r}\n"
        "with open(path, 'a') as record:\n"
        "    record.write('gap failed\\n' if failed else 'gap\\n')\n"
        "sys.exit(1 if failed else 0)\n")
    if failed_tee:
        tee = root / "bin/tee"
        tee.write_text(f"#!{sys.executable}\nimport sys\nsys.stdin.read()\nsys.exit(1)\n")
        tee.chmod(0o755)
    pnpm = root / "bin/pnpm"
    pnpm.write_text(
        f"#!{sys.executable}\n"
        "import json, sys, time\n"
        "from pathlib import Path\n"
        "root = Path(__file__).resolve().parent.parent\n"
        "record = root / 'record'\n"
        "runs = [line for line in record.read_text().splitlines() if line.startswith('run')]\n"
        "answer = json.loads((root / 'answers.json').read_text())[len(runs)]\n"
        "with open(record, 'a') as file:\n"
        "    file.write('run ' + ' '.join(sys.argv[1:]) + '\\n')\n"
        "sys.stdout.write(answer[0])\n"
        "sys.stdout.flush()\n"
        "if answer[1] is None:\n"
        "    time.sleep(60)\n"
        "sys.exit(answer[1])\n")
    pnpm.chmod(0o755)
    command = ["sh", str(root / "scripts/browser-vitest.sh"), "--project", "browser-exact-dist", "a.integration.test.ts"]
    env = {"PATH": f"{root / 'bin'}{os.pathsep}{os.environ['PATH']}"}
    return root, command, env


class BrowserVitestTests(unittest.TestCase):
    def browser_vitest(self, answers, before=None, **options):
        """Run scripts/browser-vitest.sh with fake `pnpm exec vitest` answers. Return the code, output, and record."""
        root, command, env = fake_browser_vitest_root(answers, **options)
        if before:
            before(root)
        result = subprocess.run(command, capture_output=True, text=True, timeout=60, env={**os.environ, **env})
        run = f"run --dir {root}/apps/web exec vitest run --project browser-exact-dist a.integration.test.ts"
        lines = (root / "record").read_text().splitlines() if (root / "record").exists() else []
        record = ["run" if line == run else line for line in lines]
        # The temporary output of the script is under target/check, and the script removes it.
        self.assertEqual(list((root / "target/check").iterdir()), [])
        return result.returncode, result.stdout, record

    def test_only_a_run_that_connected_to_no_browser_runs_one_more_time(self):
        line = runner_module().SECOND_RUN_LINE
        one, two = ["gap", "run"], ["gap", "run", "gap", "run"]
        cases = {
            "no browser, then a pass": ([[NO_BROWSER, 1], [PASSED, 0]], (0, two, 1)),
            "no browser two times": ([[NO_BROWSER, 1], [NO_BROWSER, 1], [PASSED, 0]], (1, two, 1)),
            "a failed test": ([[FAILED_TEST, 1], [PASSED, 0]], (1, one, 0)),
            "a pass": ([[PASSED, 0], [PASSED, 0]], (0, one, 0)),
            "the connect line with tests that ran": ([[CONNECT_LINE_WITH_TESTS, 1], [PASSED, 0]], (1, one, 0)),
        }
        for name, (answers, expected) in cases.items():
            with self.subTest(name):
                code, output, record = self.browser_vitest(answers)
                self.assertEqual((code, record, output.splitlines().count(line)), expected, output)
                # The caller gets the output of each run.
                self.assertEqual(output.replace(f"{line}\n", ""),
                                 "".join(text for text, _ in answers[:len(expected[1]) // 2]))

    def test_a_run_whose_start_or_output_fails_fails_with_no_second_run(self):
        line = runner_module().SECOND_RUN_LINE
        cases = {
            # The event line comes only for a second run that starts.
            "the second wait for the start gap fails": (
                {"answers": [[NO_BROWSER, 1], [PASSED, 0]], "failed_gaps": {2}}, ["gap", "run", "gap failed"]),
            "the output is not kept": ({"answers": [[PASSED, 0]], "failed_tee": True}, ["gap", "run"]),
            "no temporary directory": (
                {"answers": [[PASSED, 0]], "before": lambda root: (root / "target/check").chmod(0o555)}, []),
        }
        for name, (options, record) in cases.items():
            with self.subTest(name):
                code, output, actual = self.browser_vitest(**options)
                self.assertEqual((code, actual, output.splitlines().count(line)), (1, record, 0), output)

    def test_a_stopped_run_removes_its_temporary_output(self):
        root, command, env = fake_browser_vitest_root([["started\n", None]])
        process = subprocess.Popen(command, stdout=subprocess.PIPE, text=True, env={**os.environ, **env},
                                   start_new_session=True)
        try:
            self.assertEqual(process.stdout.readline(), "started\n")
            self.assertEqual(len(list((root / "target/check").iterdir())), 1)
            # The lane runner sends SIGTERM to the process group of a lane.
            os.killpg(process.pid, signal.SIGTERM)
            self.assertEqual(process.wait(timeout=30), 143)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
        self.assertEqual(list((root / "target/check").iterdir()), [])

    def test_the_runner_counts_the_second_runs_of_the_script_in_the_lanes_of_its_run(self):
        module = runner_module()
        def failed_final_check():
            raise ValueError("a final check failed")

        for code, answers, final_checks in ((0, [[NO_BROWSER, 1], [PASSED, 0]], ()),
                                            (1, [[NO_BROWSER, 1], [NO_BROWSER, 1]], ()),
                                            (1, [[NO_BROWSER, 1], [PASSED, 0]], (failed_final_check,))):
            with self.subTest(code=code, final_checks=len(final_checks)):
                root, command, env = fake_browser_vitest_root(answers)
                # target/check keeps the logs of the lanes of earlier runs.
                (root / "target/check/lanes-test-old.log").write_text(f"{module.SECOND_RUN_LINE}\n")
                lanes = [module.Lane("lanes-test-second-run", command, env=env),
                         module.Lane("lanes-test-one-run", ["true"])]
                result, text, _ = run_lanes(module, lanes, root, final_checks)
                self.assertEqual((result, text.splitlines()[-1]),
                                 (code, "check-lanes: second runs of a browser Vitest run: 1 (lanes-test-second-run 1)"),
                                 text)


if __name__ == "__main__":
    unittest.main()
