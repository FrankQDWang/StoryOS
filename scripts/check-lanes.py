#!/usr/bin/env python3
"""Run the lanes of a check tier at the same time (ADR 0070). Each lane writes target/check/<lane>.log.

A lane with AFTER names starts when those lanes passed. COMMAND runs with `sh -ec` in the repository root.
--browser-source adds the lanes of the browser-source Vitest project, one browser process for each lane.
--postgres-shards adds the ignored tests of the library test binaries in shards, each with its own database.
When a lane fails, the runner stops the other lanes, prints the log of the failed lane, and exits with 1.
"""

import json
import os
from pathlib import Path
import re
import shlex
import signal
import socket
import subprocess
import sys
import time

# Selected from measured runs of `make check` on an Apple M4 with 10 cores and 32 GB (issue 1711).
LANE_LIMIT = 6
POSTGRES_SHARDS = 4
# This file takes approximately 204 s, so it gets its own browser process, and the other files share one.
# Four browser processes did not connect within the Vitest timeout in 3 of 6 runs of `make check` on 2026-11-19.
SEPARATE_BROWSER_SOURCE_FILES = ("test/browser-source/journal-working-set.integration.test.ts",)
# Google Chrome can fail to start when two instances start at the same time, so the browser lanes start
# with this gap: in a test on 2026-11-19, 1 of 20 starts at the same time failed, and 0 of 40 with a gap.
BROWSER_START_GAP_SECONDS = 8
# Each browser lane gets its own port for its Vite server (STORYOS_BROWSER_API_PORT in apps/web/vitest.config.ts).
BROWSER_PORTS = range(41000, 41100)
# A stopped lane removes its database container in this time.
STOP_SECONDS = 30
USAGE = ("usage: scripts/check-lanes.py "
         "[--lane NAME[:AFTER,...]=COMMAND | --browser-source | --postgres-shards AFTER,...]...")
STOP_SIGNALS = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
LANE_ARGUMENT = re.compile(r"([a-z0-9-]+)(?::([a-z0-9,-]+))?=(.+)", re.DOTALL)


class Lane:
    """A command with its own log. `passed` reads the log of a passed lane and returns more lanes."""

    def __init__(self, name, command, after=(), env=None, passed=None, browser=False):
        self.name, self.command, self.after, self.env, self.passed = name, command, tuple(after), env, passed
        self.browser = browser
        self.process = self.log = None
        self.started = self.duration = None


def free_browser_port(taken):
    """A port of 127.0.0.1 below the ephemeral range, so that no outgoing connection takes it before Vite."""
    for port in BROWSER_PORTS:
        if port in taken:
            continue
        with socket.socket() as candidate:
            try:
                candidate.bind(("127.0.0.1", port))
            except OSError:
                continue
        return port
    raise ValueError(f"no free browser port from {BROWSER_PORTS.start} to {BROWSER_PORTS.stop - 1}")


def vitest_lane(root, name, project, files):
    web = root / "apps/web"
    output = root / "target/check" / f"{name}.json"

    def passed(_log):
        suites = json.loads(output.read_text()).get("testResults", [])
        if (sorted(suite["name"] for suite in suites) != sorted(str(web / file) for file in files)
                or any(not any(test["status"] == "passed" for test in suite["assertionResults"])
                       for suite in suites)):
            raise ValueError(f"{name}: a listed file did not run or has no passing test")
        return []

    return Lane(name, ["pnpm", "--dir", "apps/web", "exec", "vitest", "run", "--project", project, *files,
                       "--passWithNoTests=false", "--allowOnly=false", "--reporter=default", "--reporter=json",
                       f"--outputFile={output}"], passed=passed, browser=True)


def browser_source_lanes(root):
    web = root / "apps/web"
    files = sorted(str(path.relative_to(web)) for path in (web / "test/browser-source").rglob("*.test.ts"))
    missing = [file for file in SEPARATE_BROWSER_SOURCE_FILES if file not in files]
    if missing:
        raise ValueError(f"browser-source: a separate file does not exist: {', '.join(missing)}")
    groups = [[file] for file in SEPARATE_BROWSER_SOURCE_FILES]
    groups.append([file for file in files if file not in SEPARATE_BROWSER_SOURCE_FILES])
    if sorted(file for group in groups for file in group) != files:
        raise ValueError("browser-source: a file of the project is in no lane or in more than one lane")
    names = [f"browser-{Path(file).name.removesuffix('.test.ts').removesuffix('.integration')}"
             for file in SEPARATE_BROWSER_SOURCE_FILES]
    return [vitest_lane(root, name, "browser-source", group) for name, group in zip([*names, "browser-source"], groups)]


def postgres_lanes(root, after, final_checks):
    """A plan lane builds the list of the ignored tests; then one lane for each shard runs its part."""
    shards = []

    def plan(log):
        tests = []
        for line in log.splitlines():
            if not line.startswith("{"):
                continue
            message = json.loads(line)
            target = message.get("target", {})
            if (message.get("reason") != "compiler-artifact" or target.get("kind") != ["lib"]
                    or not message["profile"]["test"] or not message.get("executable")):
                continue
            binary, directory = message["executable"], str(Path(message["manifest_path"]).parent)
            listed = subprocess.run([binary, "--list", "--ignored"], cwd=directory, check=True,
                                    capture_output=True, text=True).stdout
            tests.extend((directory, binary, name) for name in re.findall(r"^(\S+): test$", listed, re.MULTILINE))
        names = [name for _, _, name in tests]
        if not names or len(set(names)) != len(names):
            raise ValueError("postgres: the list of the ignored tests is empty or has a name two times")
        final_checks.append(lambda: compare_postgres_counts(root, shards, len(names)))
        print(f"postgres: {len(names)} ignored tests in {POSTGRES_SHARDS} shards", flush=True)
        lanes = []
        for index in range(POSTGRES_SHARDS):
            part = tests[index::POSTGRES_SHARDS]
            commands = []
            for directory, binary in dict.fromkeys((directory, binary) for directory, binary, _ in part):
                selected = [name for test_directory, test_binary, name in part
                            if (test_directory, test_binary) == (directory, binary)]
                commands.append(f"cd {shlex.quote(directory)}; {shlex.quote(binary)} --ignored --nocapture "
                                f"--exact {shlex.join(selected)}")
            name = f"postgres-{index + 1}"
            shards.append(name)
            lanes.append(Lane(name, [str(root / "scripts/dev-postgres.sh"), "with", "sh", "-ec", "; ".join(commands)]))
        return lanes

    return [Lane("postgres-plan", ["cargo", "test", "--workspace", "--all-targets", "--all-features", "--no-run",
                                   "--message-format=json"], after=after, passed=plan)]


def compare_postgres_counts(root, shards, listed):
    passed = sum(int(count) for shard in shards for count in re.findall(
        r"^test result: \S+ (\d+) passed;", (root / "target/check" / f"{shard}.log").read_text(), re.MULTILINE))
    if passed != listed:
        raise ValueError(f"postgres: {passed} ignored tests passed, but the test binaries list {listed}")
    print(f"postgres: {passed} of {listed} listed ignored tests passed", flush=True)


def parse(root, arguments, final_checks):
    lanes = []
    arguments = list(arguments)
    while arguments:
        option = arguments.pop(0)
        if option == "--browser-source":
            lanes.extend(browser_source_lanes(root))
        elif option in ("--lane", "--postgres-shards") and arguments:
            value = arguments.pop(0)
            if option == "--postgres-shards":
                lanes.extend(postgres_lanes(root, value.split(","), final_checks))
                continue
            match = LANE_ARGUMENT.fullmatch(value)
            if not match:
                raise ValueError(f"invalid lane: {value}")
            name, after, command = match.groups()
            lanes.append(Lane(name, ["sh", "-ec", command], after=after.split(",") if after else ()))
        else:
            raise ValueError(USAGE)
    names = [lane.name for lane in lanes]
    if len(set(names)) != len(names):
        raise ValueError("two lanes have the same name")
    if any(dependency not in names for lane in lanes for dependency in lane.after):
        raise ValueError("a lane waits for a lane that does not exist")
    return lanes


def start(root, lane, running):
    if lane.browser:
        port = free_browser_port({int(other.env["STORYOS_BROWSER_API_PORT"]) for other in running if other.browser})
        lane.env = {"STORYOS_BROWSER_API_PORT": str(port)}
    lane.log = open(root / "target/check" / f"{lane.name}.log", "w")
    lane.started = time.monotonic()
    lane.process = subprocess.Popen(lane.command, cwd=root, env={**os.environ, **(lane.env or {})},
                                    stdin=subprocess.DEVNULL,
                                    stdout=lane.log, stderr=subprocess.STDOUT, start_new_session=True)
    print(f"Lane {lane.name}: started", flush=True)


def group_exists(lane):
    try:
        os.killpg(lane.process.pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def signal_group(lane, stop_signal):
    try:
        os.killpg(lane.process.pid, stop_signal)
    except ProcessLookupError:
        pass


def stop(lanes):
    """Stop the process group of each started lane, and give its traps the time to remove their containers."""
    for stop_signal in STOP_SIGNALS:
        signal.signal(stop_signal, signal.SIG_IGN)
    for lane in lanes:
        signal_group(lane, signal.SIGTERM)
    deadline = time.monotonic() + STOP_SECONDS
    while time.monotonic() < deadline:
        for lane in lanes:
            lane.process.poll()
        if not any(group_exists(lane) for lane in lanes):
            return
        time.sleep(0.2)
    for lane in lanes:
        if group_exists(lane):
            print(f"Lane {lane.name}: killed after {STOP_SECONDS} s", flush=True)
            signal_group(lane, signal.SIGKILL)


def print_failure(lane, reason):
    print(f"Lane {lane.name}: FAIL after {round(lane.duration)} s ({reason}). The other lanes are stopped.")
    print(f"The log {lane.log.name}:")
    print(Path(lane.log.name).read_text(errors="replace"), flush=True)


def run(root, lanes, final_checks):
    (root / "target/check").mkdir(parents=True, exist_ok=True)
    pending, running, finished, started = list(lanes), [], [], []
    began = time.monotonic()
    last_browser_start = began - BROWSER_START_GAP_SECONDS
    # A signal only sets the flag, so that each started lane is in `started` when the runner stops the lanes.
    interrupted = []
    for stop_signal in STOP_SIGNALS:
        signal.signal(stop_signal, lambda number, _frame: interrupted.append(number))
    try:
        while (pending or running) and not interrupted:
            ready = [lane for lane in pending
                     if all(dependency in {done.name for done in finished} for dependency in lane.after)]
            if not running and not ready:
                raise ValueError("no lane can start")
            for lane in ready:
                if len(running) >= LANE_LIMIT or interrupted:
                    break
                if lane.browser and time.monotonic() - last_browser_start < BROWSER_START_GAP_SECONDS:
                    continue
                if lane.browser:
                    last_browser_start = time.monotonic()
                pending.remove(lane)
                start(root, lane, running)
                started.append(lane)
                running.append(lane)
            time.sleep(0.2)
            for lane in [lane for lane in running if not interrupted and lane.process.poll() is not None]:
                running.remove(lane)
                lane.duration = time.monotonic() - lane.started
                lane.log.close()
                reason = f"exit code {lane.process.returncode}"
                if lane.process.returncode == 0:
                    try:
                        pending[:0] = lane.passed(Path(lane.log.name).read_text()) if lane.passed else []
                        finished.append(lane)
                        print(f"Lane {lane.name}: PASS {round(lane.duration)} s", flush=True)
                        continue
                    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
                        reason = str(error)
                stop(started)
                print_failure(lane, reason)
                return 1
        if interrupted:
            stop(started)
            print("check-lanes: interrupted. The lanes are stopped.", flush=True)
            return 130
        for check in final_checks:
            check()
    except (ValueError, OSError, KeyError, TypeError) as error:
        stop(started)
        print(f"check-lanes: {error}", file=sys.stderr, flush=True)
        return 1
    for lane in finished:
        print(f"Lane {lane.name}: {round(lane.duration)} s, started at {round(lane.started - began)} s")
    return 0


def main():
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    final_checks = []
    try:
        lanes = parse(root, sys.argv[1:], final_checks)
    except (ValueError, OSError) as error:
        print(f"check-lanes: {error}", file=sys.stderr)
        return 1
    return run(root, lanes, final_checks)


if __name__ == "__main__":
    sys.exit(main())
