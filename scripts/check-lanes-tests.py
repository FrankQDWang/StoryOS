#!/usr/bin/env python3
"""Tests of the stop of scripts/check-lanes.py (issue 1711). Each test runs the runner with lanes of its own.

CHECK_LANES_SCRIPT gives another runner to test, for example the runner of an earlier commit.
"""

import hashlib
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


def containers():
    return subprocess.run(["docker", "ps", "-a", "--format", "{{.Names}}"], capture_output=True, text=True,
                          check=True).stdout.split()


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

    def test_the_stop_removes_only_the_containers_of_its_own_run(self):
        own, unlabelled, other_run = (f"storyos-check-{CHECKOUT_ID}-{kind}-{self.marker}"
                                      for kind in ("own", "unlabelled", "other-run"))
        try:
            subprocess.run(["docker", "create", "--name", unlabelled, IMAGE], capture_output=True, check=True)
            subprocess.run(["docker", "create", "--name", other_run, "--label", "storyos.check-run=another-run",
                            IMAGE], capture_output=True, check=True)
            created = self.directory / "created"
            process = runner(
                f"lanes-test-container=docker create --name {own} "
                f"--label \"storyos.check-run=$STORYOS_CHECK_RUN\" {IMAGE} >/dev/null; touch {created}; {self.sleep}",
                f"lanes-test-container-fails={wait_for(created)}; exit 1")
            output, _ = process.communicate(timeout=120)
            self.assertEqual(process.returncode, 1, output)
            remaining = containers()
            self.assertNotIn(own, remaining, output)
            self.assertIn(unlabelled, remaining, output)
            self.assertIn(other_run, remaining, output)
            self.assertEqual(pids_of(self.sleep), [], output)
        finally:
            subprocess.run(["docker", "rm", "-fv", own, unlabelled, other_run], capture_output=True)

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


if __name__ == "__main__":
    unittest.main()
