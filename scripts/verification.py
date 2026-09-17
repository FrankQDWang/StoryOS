#!/usr/bin/env python3
"""Classify verification inputs and record command results for one source tree."""

import argparse
from datetime import datetime, timezone
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import time
import uuid


def git(root, *arguments):
    return subprocess.check_output(["git", *arguments], cwd=root, text=True).strip()


def source_identity(root):
    stamps = []
    for path in input_paths(root):
        metadata = (root / path).stat()
        stamps.append((path, metadata.st_ino, metadata.st_size,
                       metadata.st_mtime_ns, metadata.st_ctime_ns))
    return {"commit": git(root, "rev-parse", "HEAD"),
            "tree": git(root, "rev-parse", "HEAD^{tree}"),
            "write_stamps_sha256": hashlib.sha256(json.dumps(stamps).encode()).hexdigest(),
            "dirty": bool(git(root, "status", "--porcelain", "--untracked-files=all"))}


def input_paths(root):
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root,
    ).decode().split("\0")
    return sorted(set(paths) - {""})


def inventory(root):
    policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
    if policy.get("version") != 1 or not policy.get("rules"):
        raise ValueError("Unsupported or empty verification policy")
    for rule in policy["rules"]:
        if set(rule) != {"pattern", "kind", "group"} or not all(
                isinstance(value, str) and value for value in rule.values()):
            raise ValueError("Each input rule needs a pattern, kind, and group")
    files, errors = [], []
    for path in input_paths(root):
        rule = next((r for r in policy["rules"] if fnmatch.fnmatchcase(path, r["pattern"])), None)
        is_test = re.search(r"(?:_tests?\.(?:rs|py)|\.(?:test|spec)\.[cm]?[jt]sx?|/tests/.*\.rs|/test_[^/]+\.py)$", path)
        test_directory = path.startswith("apps/web/test/")
        if (rule is None or (is_test and not rule["kind"].endswith("-test"))
                or (test_directory and rule["kind"] not in {"web-test", "fixture"})):
            errors.append(path)
            continue
        group = rule["group"]
        if group == "cargo":
            crate = path.split("/")[1]
            if not (root / "crates" / crate / "Cargo.toml").is_file():
                errors.append(path)
                continue
            group = f"cargo:{crate}"
        files.append({"path": path, "kind": rule["kind"], "group": group})
    if errors:
        raise ValueError("Unclassified inputs or unsupported test locations:\n" + "\n".join(errors))
    return {"version": 1, "files": files}


def write_json(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def execute(command, environment, new_group):
    interrupted = 0
    try:
        child = subprocess.Popen(command, env=environment, start_new_session=new_group)
    except OSError as error:
        print(str(error), file=sys.stderr)
        return 127, interrupted

    def interrupt(signum, _frame):
        nonlocal interrupted
        interrupted = signum
        if new_group:
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass

    previous = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        code = child.wait()
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    return code, interrupted


def step(root, stage, command):
    if not re.fullmatch(r"[a-z][a-z0-9-]*", stage):
        raise ValueError("Stage names use lowercase words separated by hyphens")
    run = os.environ.get("STORYOS_VERIFICATION_RUN")
    if run is None:
        code, interrupted = execute(command, os.environ.copy(), os.name == "posix")
        return 128 + interrupted if interrupted else (code if code >= 0 else 128 - code)
    directory = Path(run).resolve()
    if directory.parent != (root / "target/verification").resolve() or not directory.is_dir():
        raise ValueError("The verification run must be inside target/verification")
    identifier = uuid.uuid4().hex
    path = directory / "steps" / f"{identifier}.json"
    started = time.monotonic()
    result = {"id": identifier, "stage": stage, "command": command,
              "parent": os.environ.get("STORYOS_VERIFICATION_PARENT"),
              "started_monotonic": started, "status": "running"}
    write_json(path, result)
    code, interrupted = execute(command, {**os.environ, "STORYOS_VERIFICATION_PARENT": identifier}, False)
    result.update(duration_seconds=time.monotonic() - started, exit_code=code,
                  status="interrupted" if interrupted else ("passed" if code == 0 else "failed"))
    write_json(path, result)
    return 128 + interrupted if interrupted else (code if code >= 0 else 128 - code)


def run(root, command):
    if os.environ.get("STORYOS_VERIFICATION_RUN"):
        raise ValueError("A complete verification run cannot be nested")
    started = time.monotonic()
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    directory = root / "target/verification" / f"{timestamp}-{uuid.uuid4().hex[:8]}"
    (directory / "steps").mkdir(parents=True)
    report_path = directory / "report.json"
    report = {"version": 1, "started_at": timestamp, "command": command, "status": "running",
              "environment": {"system": platform.system(), "machine": platform.machine(),
                              "python": platform.python_version()}}
    write_json(report_path, report)
    code, interrupted = 1, 0
    try:
        report["source_start"] = source_identity(root)
        if report["source_start"]["dirty"]:
            raise ValueError("Complete verification requires a clean tracked and untracked worktree")
        report["inventory"] = inventory(root)
        code, interrupted = execute(command, {**os.environ, "STORYOS_VERIFICATION_RUN": str(directory)},
                                    os.name == "posix")
        report["source_end"] = source_identity(root)
        report["status"] = "passed" if code == 0 else "failed"
        if report["source_end"] != report["source_start"]:
            report["status"] = "source-changed"
        if interrupted:
            report["status"] = "interrupted"
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
    steps = [json.loads(path.read_text()) for path in (directory / "steps").glob("*.json")]
    report["steps"] = sorted(steps, key=lambda item: item["started_monotonic"])
    if report["status"] == "passed" and (not steps or any(item["status"] != "passed" for item in steps)):
        report["status"] = "incomplete"
    report.update(duration_seconds=time.monotonic() - started, exit_code=code)
    write_json(report_path, report)
    print(f"Verification {report['status']}: {report['duration_seconds']:.2f}s; report: {report_path}", flush=True)
    if report["status"] == "passed":
        return 0
    return 128 + interrupted if interrupted else (code if code > 0 else 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    commands.add_parser("inventory").add_argument("--check", action="store_true")
    for action in ("run", "step"):
        command_parser = commands.add_parser(action)
        if action == "step":
            command_parser.add_argument("stage")
        command_parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    try:
        root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel"))
        if arguments.action == "inventory":
            result = inventory(root)
            print(f"Verified ownership of {len(result['files'])} input files" if arguments.check
                  else json.dumps(result, indent=2))
            return 0
        command = arguments.command
        if command[:1] == ["--"]:
            command = command[1:]
        if not command:
            raise ValueError("A verification command is required")
        return run(root, command) if arguments.action == "run" else step(root, arguments.stage, command)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
