#!/usr/bin/env python3
"""Run discovered verification-tool test files with isolated, bounded workers."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

import verification


def run(root):
    policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
    files = sorted(item["path"] for item in verification.inventory(root)["files"]
                   if item["kind"] == "verification-test" and item["group"] == "verification-tools"
                   and (root / item["path"]).is_file())
    if not files:
        raise ValueError("No verification-tool test files were discovered")
    parallel = policy.get("verification_test_parallel_files", [])
    limit = policy.get("verification_test_workers", 1)
    if any(path not in files for path in parallel):
        raise ValueError("Invalid verification-tool file budget or independence declaration")
    requested = os.environ.get("STORYOS_VERIFICATION_TEST_WORKERS")
    if requested is not None:
        if not requested.isdecimal() or not 1 <= int(requested) <= limit:
            raise ValueError("Diagnostic worker budget must be between 1 and the policy limit")
        limit = int(requested)
    if not os.environ.get("STORYOS_VERIFICATION_RUN"):
        raise ValueError("Use a managed verification command for file test evidence")

    active = []
    failures = []
    interrupted = 0

    def terminate(signum, _frame):
        nonlocal interrupted
        interrupted = signum
        for process, _ in active:
            try:
                os.killpg(process.pid, signum)
            except ProcessLookupError:
                pass

    def finish(*, one=False):
        deadline = None
        while active:
            if interrupted and deadline is None:
                deadline = time.monotonic() + 5
            completed = [(process, temporary) for process, temporary in active if process.poll() is not None]
            for process, temporary in completed:
                active.remove((process, temporary))
                code = process.wait()
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                else:
                    end = time.monotonic() + 5
                    while time.monotonic() < end:
                        try:
                            os.killpg(process.pid, 0)
                        except ProcessLookupError:
                            break
                        time.sleep(0.05)
                    else:
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        failures.append(1)
                temporary.cleanup()
                if code:
                    failures.append(code if code > 0 else 128 - code)
            if completed and one:
                return
            if interrupted and deadline is not None and time.monotonic() >= deadline:
                for process, _ in active:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            if not completed:
                time.sleep(0.05)

    previous = {sig: signal.signal(sig, terminate) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        for path in files:
            if interrupted:
                break
            if path not in parallel:
                finish()
                if interrupted:
                    break
            elif len(active) >= limit:
                finish(one=True)
                if interrupted:
                    break
            temporary = tempfile.TemporaryDirectory(prefix="storyos-verification-file-")
            environment = os.environ.copy()
            environment.pop("CARGO_TARGET_DIR", None)
            environment.pop("STORYOS_RUST_CACHE_ROOT", None)
            environment.update(TMPDIR=temporary.name, TMP=temporary.name, TEMP=temporary.name,
                               PYTHONDONTWRITEBYTECODE="1")
            command = [sys.executable, str(Path(verification.__file__).resolve()), "step",
                       "--node-id", f"file:verification-tools:{path}", "--node-only", "verification-tests", "--",
                       sys.executable, "-m", "unittest", "discover", "-s", "scripts", "-p", Path(path).name]
            try:
                process = subprocess.Popen(command, cwd=root, env=environment, start_new_session=True)
            except OSError as error:
                temporary.cleanup()
                print(str(error), file=sys.stderr)
                failures.append(127)
                continue
            active.append((process, temporary))
            if path not in parallel:
                finish()
        finish()
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        for process, temporary in active:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            temporary.cleanup()
    return 128 + interrupted if interrupted else (failures[0] if failures else 0)


if __name__ == "__main__":
    try:
        sys.exit(run(Path.cwd()))
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
