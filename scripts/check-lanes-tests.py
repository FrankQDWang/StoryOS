#!/usr/bin/env python3
"""Tests of scripts/check-lanes.py (issues 1711, 1712): the stop, the Chrome start gap, and some functions.

CHECK_LANES_SCRIPT gives another runner to test, for example the runner of an earlier commit.
"""

import hashlib
import importlib.util
import os
from pathlib import Path
import re
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


def runner_module():
    """Load the runner as a module, for the tests of its functions."""
    spec = importlib.util.spec_from_file_location("check_lanes", RUNNER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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


if __name__ == "__main__":
    unittest.main()
