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
IMAGE = re.search(r"^storyos_postgres_image=(\S+)$", (ROOT / "scripts/lib/controlled-postgres.sh").read_text(),
                  re.MULTILINE).group(1)


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


class StopTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="storyos-check-lanes-"))
        self.marker = uuid.uuid4().hex[:12]

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
                f"--label \"storyos.check-run=$STORYOS_CHECK_RUN\" {IMAGE} >/dev/null; touch {created}; sleep 300",
                f"lanes-test-container-fails={wait_for(created)}; exit 1")
            output, _ = process.communicate(timeout=120)
            self.assertEqual(process.returncode, 1, output)
            remaining = containers()
            self.assertNotIn(own, remaining, output)
            self.assertIn(unlabelled, remaining, output)
            self.assertIn(other_run, remaining, output)
        finally:
            subprocess.run(["docker", "rm", "-fv", own, unlabelled, other_run], capture_output=True)

    def test_the_stop_reaches_a_detached_child_after_its_lane_ended(self):
        child_pid = self.directory / "child-pid"
        spawn = ("import subprocess, sys; child = subprocess.Popen([sys.executable, '-c', "
                 "'import time; time.sleep(300)'], start_new_session=True); "
                 f"open('{child_pid}', 'w').write(str(child.pid))")
        process = runner(f"lanes-test-detached={sys.executable} -c \"{spawn}\"",
                         f"lanes-test-detached-fails={wait_for(child_pid)}; sleep 1.5; exit 1")
        output, _ = process.communicate(timeout=120)
        pid = int(child_pid.read_text())
        try:
            self.assertEqual(process.returncode, 1, output)
            deadline = time.monotonic() + 5
            while alive(pid) and time.monotonic() < deadline:
                time.sleep(0.1)
            self.assertFalse(alive(pid), f"the detached child {pid} of the ended lane still runs\n{output}")
        finally:
            if alive(pid):
                os.kill(pid, signal.SIGKILL)

    def test_an_interruption_during_the_stop_does_not_end_the_stop(self):
        ready = self.directory / "ready"
        # The trap keeps the lane for 2 s after SIGTERM, so the runner scans the processes during that time.
        process = runner(f"lanes-test-slow=trap 'sleep 2; exit 143' TERM; touch {ready}; sleep 300 & wait",
                         start_new_session=True)
        deadline = time.monotonic() + 60
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.1)
        # Ctrl-C in a terminal sends SIGINT to each process of the foreground process group.
        end = time.monotonic() + 3
        while time.monotonic() < end and process.poll() is None:
            os.killpg(process.pid, signal.SIGINT)
            time.sleep(0.01)
        output, _ = process.communicate(timeout=120)
        self.assertEqual(process.returncode, 130, output)
        self.assertIn("check-lanes: interrupted. The lanes are stopped.", output)
        self.assertNotIn("Traceback", output)


if __name__ == "__main__":
    unittest.main()
