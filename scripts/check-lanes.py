#!/usr/bin/env python3
"""Run the lanes of a check tier at the same time (ADR 0070). Each lane writes target/check/<lane>.log.

A lane with AFTER names starts when those lanes passed. COMMAND runs with `sh -ec` in the repository root.
--browser-source adds the lanes of the browser-source Vitest project, one browser process for each lane.
--postgres-shards adds the ignored tests of the library test binaries in shards, each with its own database.
--project-scope adds the parts of `make check-full` after the release package: the storage activation checks, the
exact-dist journeys in shards, and the two modes of the recovery drill. It needs the lanes `build` and `package`.
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
import uuid

# Selected from measured runs of `make check` on an Apple M4 with 10 cores and 32 GB (issue 1711).
LANE_LIMIT = 6
POSTGRES_SHARDS = 4
# This file takes approximately 204 s, so it gets its own browser process, and the other files share one.
# Four browser processes did not connect within the Vitest timeout in 3 of 6 runs of `make check` on 2026-11-19.
SEPARATE_BROWSER_SOURCE_FILES = ("test/browser-source/journal-working-set.integration.test.ts",)
# Google Chrome can fail to start when two instances start at the same time, so the browser lanes start
# with this gap: in a test on 2026-11-19, 1 of 20 starts at the same time failed, and 0 of 40 with a gap.
BROWSER_START_GAP_SECONDS = 8
# The count of the exact-dist journey shards. Each shard has its own database and its own packaged Servers.
JOURNEY_SHARDS = 3
# The time of the longest journey files in a run alone on 2026-11-19 (issue 1712). The other files take
# approximately this time each. The runner puts each file into the shard with the shortest sum of times.
LONG_JOURNEY_SECONDS = {
    "test/browser-exact-dist/s2-long-session.integration.test.ts": 85,
    "test/browser-exact-dist/production-host.integration.test.ts": 45,
    "test/browser-exact-dist/stage7-journey.integration.test.ts": 31,
}
OTHER_JOURNEY_SECONDS = 3
# scripts/verify-recovery-hold.sh runs this file after its restore (STORYOS_PHYSICAL_DRILL in apps/web/vitest.config.ts).
PHYSICAL_DRILL_FILE = "test/browser-exact-dist/s2-physical-drill.integration.test.ts"
RECOVERY_DRILLS = ("fixture-only", "mixed")
RECOVERY_DRILL_END = "Isolated restore continued writing after Recovery Visibility Proof"
# Each browser lane gets its own port for its Vite server (STORYOS_BROWSER_API_PORT in apps/web/vitest.config.ts).
BROWSER_PORTS = range(41000, 41100)
# A stopped lane removes its database container in this time.
STOP_SECONDS = 30
# Each lane gets the token of the run in this variable. scripts/lib/controlled-postgres.sh puts the token on each
# container as a label, so that the runner removes only the containers of its own run.
RUN_VARIABLE = "STORYOS_CHECK_RUN"
RUN_LABEL = "storyos.check-run"
# After SIGKILL, the runner waits this time for the processes to end.
KILL_SECONDS = 5
USAGE = ("usage: scripts/check-lanes.py "
         "[--lane NAME[:AFTER,...]=COMMAND | --browser-source | --postgres-shards AFTER,...| --project-scope]...")
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


def journey_shards(root):
    """Give each exact-dist journey file to exactly one shard. Each shard has its files in the order of their names."""
    web = root / "apps/web"
    files = sorted(str(path.relative_to(web)) for path in (web / "test/browser-exact-dist").rglob("*.test.ts"))
    missing = [file for file in [PHYSICAL_DRILL_FILE, *LONG_JOURNEY_SECONDS] if file not in files]
    if missing:
        raise ValueError(f"journeys: a named file does not exist: {', '.join(missing)}")
    files.remove(PHYSICAL_DRILL_FILE)
    shards, totals = [[] for _ in range(JOURNEY_SHARDS)], [0] * JOURNEY_SHARDS
    for file in sorted(files, key=lambda file: (-LONG_JOURNEY_SECONDS.get(file, OTHER_JOURNEY_SECONDS), file)):
        index = totals.index(min(totals))
        shards[index].append(file)
        totals[index] += LONG_JOURNEY_SECONDS.get(file, OTHER_JOURNEY_SECONDS)
    if sorted(file for shard in shards for file in shard) != files:
        raise ValueError("journeys: a journey file is in no shard or in more than one shard")
    return [sorted(shard) for shard in shards]


def journey_lane(root, name, files):
    web, report = root / "apps/web", root / "target/check" / name

    def passed(_log):
        suites = [suite for output in sorted(report.parent.glob(f"{name}-*.json"))
                  for suite in json.loads(output.read_text()).get("testResults", [])]
        if (sorted(suite["name"] for suite in suites) != sorted(str(web / file) for file in files)
                or any(not any(test["status"] == "passed" for test in suite["assertionResults"])
                       for suite in suites)):
            raise ValueError(f"{name}: a journey file did not run one time or has no passing test")
        return []

    # scripts/verify-project-scope.sh writes the Vitest report of each of its runs to <report>-<run>.json.
    return Lane(name, [str(root / "scripts/verify-project-scope.sh"), "journeys", *files], after=("package",),
                env={"STORYOS_JOURNEY_REPORT": str(report)}, passed=passed, browser=True)


def recovery_drill_lane(root, mode):
    def passed(log):
        if RECOVERY_DRILL_END not in log:
            raise ValueError(f"recovery-{mode}: the log has no line '{RECOVERY_DRILL_END}'")
        return []

    return Lane(f"recovery-{mode}", [str(root / "scripts/verify-recovery-hold.sh")], after=("package",),
                env={"STORYOS_RECOVERY_DRILL": mode}, passed=passed, browser=True)


def project_scope_lanes(root):
    lanes = [journey_lane(root, f"journeys-{index + 1}", files) for index, files in enumerate(journey_shards(root))]
    lanes += [recovery_drill_lane(root, mode) for mode in RECOVERY_DRILLS]
    # The activation checks run ignored tests of the library test binary of the build lane.
    lanes.append(Lane("activation", [str(root / "scripts/verify-project-scope.sh"), "activation"],
                      after=("build", "package")))
    return lanes


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
        elif option == "--project-scope":
            lanes.extend(project_scope_lanes(root))
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


def start(root, lane, running, token):
    if lane.browser:
        port = free_browser_port({int(other.env["STORYOS_BROWSER_API_PORT"]) for other in running if other.browser})
        lane.env = {**(lane.env or {}), "STORYOS_BROWSER_API_PORT": str(port)}
    lane.log = open(root / "target/check" / f"{lane.name}.log", "w")
    lane.started = time.monotonic()
    lane.process = subprocess.Popen(lane.command, cwd=root,
                                    env={**os.environ, **(lane.env or {}), RUN_VARIABLE: token},
                                    stdin=subprocess.DEVNULL,
                                    stdout=lane.log, stderr=subprocess.STDOUT, start_new_session=True)
    print(f"Lane {lane.name}: started", flush=True)


def tool_output(command):
    """Run ps or docker in a session of its own, so that an interruption of the run does not reach it."""
    return subprocess.run(command, capture_output=True, text=True, check=True, start_new_session=True).stdout


def process_table():
    """Return {pid: (parent, group, state, start time)} of each process. It reads no environment."""
    table = {}
    for line in tool_output(["ps", "-A", "-o", "pid=,ppid=,pgid=,stat=,lstart="]).splitlines():
        pid, parent, group, state, started = line.split(None, 4)
        table[int(pid)] = (int(parent), int(group), state, started.strip())
    return table


def descendants(table, leaders):
    """Return {pid: start time} of each descendant of the leaders."""
    children = {}
    for pid, (parent, _, _, _) in table.items():
        children.setdefault(parent, []).append(pid)
    snapshot, waiting = {}, list(leaders)
    while waiting:
        for child in children.get(waiting.pop(), []):
            if child not in snapshot:
                snapshot[child] = table[child][3]
                waiting.append(child)
    return snapshot


def wait_until(ended, seconds):
    """Scan the processes until `ended(table)` is true. Return False when the time ends first."""
    deadline = time.monotonic() + seconds
    while True:
        if ended(process_table()):
            return True
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.2)


def signal_processes(pids, stop_signal, kill=os.kill):
    for pid in pids:
        try:
            kill(pid, stop_signal)
        # macOS refuses a signal to a group that has only a zombie leader with EPERM.
        except (ProcessLookupError, PermissionError):
            pass


def run_containers(token):
    return tool_output(["docker", "ps", "-a", "-q", "--filter", f"label={RUN_LABEL}={token}"]).split()


def stop(lanes, token):
    """Stop each lane whose leader the runner did not reap, then remove the containers of the run that remain.

    The runner reaps a leader only at the end of the stop, so a leader keeps its ID, also as a zombie, and the
    system cannot give the ID of its process group to another process. A descendant in another process group or
    session, such as Google Chrome, is in a snapshot from before the first signal, and it gets a signal only while
    it runs with the same ID and start time. A lane that ended and passed before the stop is out of the scope of
    the stop: the runner reaped its leader. Return False when a process or a container is not verified as ended.
    """
    for stop_signal in STOP_SIGNALS:
        signal.signal(stop_signal, signal.SIG_IGN)
    leaders = {lane.process.pid for lane in lanes if lane.process.returncode is None}
    stopped = True

    def groups_ended(table):
        return not any(group in leaders and not state.startswith("Z") for _, group, state, _ in table.values())

    try:
        snapshot = descendants(process_table(), leaders)

        def survivors(table):
            return [pid for pid, started in snapshot.items()
                    if pid in table and table[pid][3] == started and not table[pid][2].startswith("Z")]

        signal_processes(leaders, signal.SIGTERM, os.killpg)
        if not wait_until(groups_ended, STOP_SECONDS):
            print(f"check-lanes: the lanes are killed after {STOP_SECONDS} s", flush=True)
            signal_processes(leaders, signal.SIGKILL, os.killpg)
            stopped = wait_until(groups_ended, KILL_SECONDS)
        for stop_signal, seconds in ((signal.SIGTERM, STOP_SECONDS), (signal.SIGKILL, KILL_SECONDS)):
            remaining = survivors(process_table())
            if not remaining:
                break
            signal_processes(remaining, stop_signal)
            if wait_until(lambda table: not survivors(table), seconds):
                break
        else:
            stopped = False
        if not stopped:
            print("check-lanes: processes of the run remain", flush=True)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"check-lanes: the processes of the run are not verified: {error}", flush=True)
        stopped = False
    finally:
        for lane in lanes:
            lane.process.poll()
    # A killed lane cannot remove its container, so the runner removes each container of the run that remains.
    try:
        remaining = run_containers(token)
        if remaining:
            tool_output(["docker", "rm", "-fv", *remaining])
            remaining = run_containers(token)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"check-lanes: the containers of the run are not verified: {error}", flush=True)
        return False
    if remaining:
        print(f"check-lanes: these containers of the run remain: {' '.join(remaining)}", flush=True)
        return False
    return stopped


def exit_code(lane):
    """Return the exit code of the lane leader when it ended, or None. The leader stays unreaped (WNOWAIT)."""
    result = os.waitid(os.P_PID, lane.process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    if result is None:
        return None
    return result.si_status if result.si_code == os.CLD_EXITED else -result.si_status


def print_failure(lane, reason):
    print(f"Lane {lane.name}: FAIL after {round(lane.duration)} s ({reason}). The other lanes are stopped.")
    print(f"The log {lane.log.name}:")
    print(Path(lane.log.name).read_text(errors="replace"), flush=True)


def run(root, lanes, final_checks):
    (root / "target/check").mkdir(parents=True, exist_ok=True)
    pending, running, finished, started = list(lanes), [], [], []
    began = time.monotonic()
    last_browser_start = began - BROWSER_START_GAP_SECONDS
    token = uuid.uuid4().hex
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
                start(root, lane, running, token)
                started.append(lane)
                running.append(lane)
            time.sleep(0.2)
            for lane in list(running):
                code = None if interrupted else exit_code(lane)
                if code is None:
                    continue
                running.remove(lane)
                lane.duration = time.monotonic() - lane.started
                lane.log.close()
                reason = f"exit code {code}"
                if code == 0:
                    try:
                        more = lane.passed(Path(lane.log.name).read_text()) if lane.passed else []
                        # A passed lane is reaped here, so it is out of the scope of a later stop.
                        lane.process.wait()
                        pending[:0] = more
                        finished.append(lane)
                        print(f"Lane {lane.name}: PASS {round(lane.duration)} s", flush=True)
                        continue
                    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
                        reason = str(error)
                stop(started, token)
                print_failure(lane, reason)
                return 1
        if interrupted:
            stopped = stop(started, token)
            print("check-lanes: interrupted. The lanes are stopped.", flush=True)
            return 130 if stopped else 1
        for check in final_checks:
            check()
    except (ValueError, OSError, KeyError, TypeError) as error:
        stop(started, token)
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
