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
import tempfile
import uuid

import verification_cache
import verification_shared
import verification_daily
import verification_status
import verification_graph
import verification_rust_cache


def git(root, *arguments):
    return subprocess.check_output(["git", *arguments], cwd=root, text=True).strip()


def source_identity(root):
    stamps, contents = [], []
    for path in input_paths(root):
        source = root / path
        if source.is_file() or source.is_symlink():
            for metadata in (source.lstat(), source.stat() if source.exists() else source.lstat()):
                stamps.append((path, metadata.st_ino, metadata.st_size,
                               metadata.st_mtime_ns, metadata.st_ctime_ns))
            contents.append((path, source.lstat().st_mode, os.readlink(source) if source.is_symlink() else None,
                             hashlib.sha256(source.read_bytes()).hexdigest() if source.is_file() else None))
        else:
            stamps.append((path, None))
            contents.append((path, None))
    return {"commit": git(root, "rev-parse", "HEAD"),
            "tree": git(root, "rev-parse", "HEAD^{tree}"),
            "write_stamps_sha256": hashlib.sha256(json.dumps(stamps).encode()).hexdigest(),
            "inputs_sha256": hashlib.sha256(json.dumps(contents).encode()).hexdigest(),
            "index_sha256": hashlib.sha256(subprocess.check_output(
                ["git", "ls-files", "--stage", "-z"], cwd=root)).hexdigest(),
            "dirty": bool(git(root, "status", "--porcelain", "--untracked-files=all"))}


def input_paths(root):
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root,
    ).decode().split("\0")
    return sorted(set(paths) - {""})


def inventory(root, revision=None):
    policy = json.loads(git(root, "show", f"{revision}:docs/agents/verification-policy.json") if revision
                        else (root / "docs/agents/verification-policy.json").read_text())
    paths = git(root, "ls-tree", "-rz", "--name-only", revision).split("\0")[:-1] if revision else input_paths(root)
    verification_daily.validate(policy)
    known = set(paths)
    if policy.get("version") != 1 or not policy.get("rules"):
        raise ValueError("Unsupported or empty verification policy")
    profiles = policy.get("file_profiles", {})
    if (not isinstance(profiles, dict) or set(profiles) - {"node-contract", "cargo"}
            or any(not isinstance(value, str) or not value for value in profiles.values())):
        raise ValueError("Unsupported file execution profile")
    if (policy.get("result_cache_profiles", []) not in ([], ["node-contract"])
            or type(policy.get("daily_workers", 2)) is not int or not 1 <= policy.get("daily_workers", 2) <= 2):
        raise ValueError("Unsupported cache profile or daily worker budget")
    for rule in policy["rules"]:
        if set(rule) != {"pattern", "kind", "group"} or not all(
                isinstance(value, str) and value for value in rule.values()):
            raise ValueError("Each input rule needs a pattern, kind, and group")
    files, errors = [], []
    for path in paths:
        rule = next((r for r in policy["rules"] if fnmatch.fnmatchcase(path, r["pattern"])), None)
        is_test = re.search(r"(?:_tests?\.(?:rs|py)|\.(?:test|spec)\.[cm]?[jt]sx?|/tests/.*\.rs|/test_[^/]+\.py)$", path)
        test_directory = path.startswith("apps/web/test/")
        if (rule is None or (is_test and not rule["kind"].endswith("-test"))
                or (test_directory and rule["kind"] not in {"web-test", "fixture"})
                or (rule["kind"] == "verification-test"
                    and not re.fullmatch(r"scripts/[A-Za-z_][A-Za-z0-9_]*_tests\.py", path))):
            errors.append(path)
            continue
        group = rule["group"]
        if group == "cargo":
            crate = path.split("/")[1]
            if f"crates/{crate}/Cargo.toml" not in known:
                errors.append(path)
                continue
            group = f"cargo:{crate}"
        files.append({"path": path, "kind": rule["kind"], "group": group})
    if errors:
        raise ValueError("Unclassified inputs or unsupported test locations; update docs/agents/verification-policy.json:\n" + "\n".join(errors))
    parallel = policy.get("verification_test_parallel_files", [])
    workers = policy.get("verification_test_workers", 1)
    eligible = {item["path"] for item in files if item["kind"] == "verification-test"
                and item["group"] == "verification-tools"}
    if (not isinstance(parallel, list) or any(not isinstance(path, str) for path in parallel)
            or len(parallel) != len(set(parallel)) or not set(parallel) <= eligible
            or type(workers) is not int or not 1 <= workers <= 4):
        raise ValueError("Invalid verification-tool file budget or independence declaration")
    if "complete" in policy:
        stages, groups = policy["complete"]["stages"], policy["complete"]["groups"]
        if (not isinstance(stages, list) or not isinstance(groups, dict) or not stages or len(stages) != len(set(stages))
                or any(not re.fullmatch(r"[a-z][a-z0-9-]*", stage) for stage in stages)
                or any(not isinstance(owners, list) or not owners or not set(owners) <= set(stages) for owners in groups.values())
                or any(item["group"].split(":")[0] not in groups for item in files
                       if item["kind"].endswith("-test") and item["kind"] not in {"historical-test", "prototype-test"})):
            raise ValueError("Incomplete verification stage or test group policy")
    if "shared_phases" in policy:
        verification_shared.plan(root, policy, files, revision)
    return {"version": 1, "files": files}


def complete_plan(root, revision="HEAD", base="origin/main", *, with_graph=False):
    policy_bytes = subprocess.check_output(["git", "show", f"{revision}:docs/agents/verification-policy.json"], cwd=root)
    profile = json.loads(policy_bytes)["complete"]
    files = [item for item in inventory(root, revision)["files"]
             if item["kind"].endswith("-test") and item["kind"] not in {"historical-test", "prototype-test"}]
    plan = {"version": 1, "base": git(root, "rev-parse", base), "tree": git(root, "rev-parse", f"{revision}^{{tree}}"),
            "policy_sha256": hashlib.sha256(policy_bytes).hexdigest(), "stages": profile["stages"],
            "test_files": sorted(item["path"] for item in files)}
    if with_graph:
        verification_graph.attach(root, plan, json.loads(policy_bytes), files, revision)
    return plan


def cargo_test_inputs(root, command):
    build = [*command, "--no-run", "--message-format=json"]
    if os.environ.get("STORYOS_VERIFICATION_RUN"):
        with tempfile.TemporaryFile(mode="w+") as output:
            code = step(root, "rust-test-build", build, stdout=output, node_only=True)
            if code:
                raise subprocess.CalledProcessError(code, build)
            output.seek(0)
            artifacts = output.read()
    else:
        artifacts = subprocess.check_output(build, cwd=root, text=True)
    compiled = set()
    for line in artifacts.splitlines():
        artifact = json.loads(line)
        if artifact.get("reason") == "compiler-artifact" and artifact["profile"]["test"] and artifact.get("executable"):
            dependencies = Path(artifact["executable"]).with_suffix(".d").read_text()
            compiled.update((root / entry[:-1].replace("\\ ", " ")).resolve() for entry in dependencies.splitlines()
                            if entry.endswith(":") and not entry.startswith("#"))
    return compiled


def rust_tests(root):
    command = ["cargo", "test", "--workspace", "--all-targets", "--all-features"]
    compiled = cargo_test_inputs(root, command)
    files = sorted(item["path"] for item in inventory(root)["files"] if item["kind"] == "rust-test")
    missing = [path for path in files if (root / path).resolve() not in compiled]
    if missing:
        raise ValueError(f"Selected files were not compiled into a test target: {missing}")
    run = os.environ.get("STORYOS_VERIFICATION_RUN")
    if run:
        write_json(Path(run) / "rust-test-files.json", files)
    return step(root, "cargo", command, node_only=True)


def write_json(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def execute(command, environment, new_group, observation=None, stdout=None):
    interrupted = 0
    try:
        child = subprocess.Popen(command, env=environment, start_new_session=new_group, stdout=stdout)
    except OSError as error:
        if observation:
            observation({"launch_error": type(error).__name__})
        print(str(error), file=sys.stderr)
        return 127, interrupted

    if observation:
        observation({"child_group": child.pid})

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
        while True:
            try:
                code = child.wait(timeout=5)
                break
            except subprocess.TimeoutExpired:
                if observation:
                    observation({"heartbeat_at": datetime.now(timezone.utc).isoformat()})
        deadline = time.monotonic() + 30
        waiting = False
        # POSIX waitpid cannot wait for reparented descendants.
        while new_group:
            try:
                os.killpg(child.pid, 0)
                if not waiting:
                    print("Waiting for verification child cleanup", flush=True)
                    waiting = True
                if time.monotonic() >= deadline:
                    os.killpg(child.pid, signal.SIGKILL)
                    code = 1
                time.sleep(0.05)
            except ProcessLookupError:
                break
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    return code, interrupted


def step(root, stage, command, *, node_id=None, stdout=None, node_only=False):
    if not re.fullmatch(r"[a-z][a-z0-9-]*", stage):
        raise ValueError("Stage names use lowercase words separated by hyphens")
    run_path = os.environ.get("STORYOS_VERIFICATION_RUN")
    if run_path is None:
        return run(root, [sys.executable, str(Path(__file__).resolve()), "step", stage, "--", *command],
                   context={"profile": "targeted", "purpose": "public-step"})
    directory = Path(run_path).resolve()
    if directory.parent != (root / "target/verification").resolve() or not directory.is_dir():
        raise ValueError("The verification run must be inside target/verification")
    identifier = uuid.uuid4().hex
    path = directory / ("nodes" if node_only else "steps") / f"{identifier}.json"
    path.parent.mkdir(exist_ok=True)
    started = time.monotonic()
    result = {"id": identifier, "stage": stage, "command": command,
              "parent": os.environ.get("STORYOS_VERIFICATION_PARENT"),
              "started_monotonic": started, "started_at": datetime.now(timezone.utc).isoformat(), "status": "running"}
    retained = json.loads((directory / "report.json").read_text())
    if node_id is None and not result["parent"] and (retained.get("plan") or {}).get("check") == stage:
        node_id = "targeted:" + stage
    result.update(verification_graph.bind_attempt(retained, stage, node_id))
    result["attempt_started"] = False
    write_json(path, result)
    def observation(fields):
        result.update(fields)
        if "child_group" in fields:
            result.update(attempt_started=True, started_at=datetime.now(timezone.utc).isoformat(),
                          started_monotonic=time.monotonic())
        write_json(path, result)
    observation({"heartbeat_at": result["started_at"]})
    code, interrupted = execute(command, {**os.environ, "STORYOS_VERIFICATION_PARENT": identifier}, False, observation, stdout)
    result.update(ended_at=datetime.now(timezone.utc).isoformat(), ended_monotonic=time.monotonic(),
                  duration_seconds=time.monotonic() - result["started_monotonic"], exit_code=code,
                  status="interrupted" if interrupted else ("passed" if code == 0 else "failed"))
    write_json(path, result)
    return 128 + interrupted if interrupted else (code if code >= 0 else 128 - code)


def record_run(root, command, *, plan=None, no_cache=False, context=None):
    if os.environ.get("STORYOS_VERIFICATION_RUN"):
        raise ValueError("A complete verification run cannot be nested")
    started = time.monotonic()
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    directory = root / "target/verification" / ((context or {}).get("run_id") or f"{timestamp}-{uuid.uuid4().hex[:8]}")
    (directory / "steps").mkdir(parents=True)
    report_path = directory / "report.json"
    report = {"version": 1, "started_at": datetime.now(timezone.utc).isoformat(), "command": command, "status": "running",
              "profile": "daily" if plan else "complete",
              "environment": {"system": platform.system(), "machine": platform.machine(),
                              "python": platform.python_version()}}
    import verification_candidate
    context = context or {}
    report.update(record_version=1, run_id=directory.name, repository=str(root.resolve()), parent=None,
                  issue=None, pr=None, purpose="daily" if plan else "explicit-command", trigger="explicit-request",
                  requested_scope=report["profile"], effective_scope=plan, started_monotonic=started,
                  process=verification_candidate.process_identity(), attempt_started=False)
    report.update(context, run_id=directory.name)
    report["requested_scope"] = report["profile"]
    report["executor_context"] = context.get("executor_context") or report["process"]["nonce"]
    report["heartbeat_at"] = datetime.now(timezone.utc).isoformat()
    def process_observation(fields):
        report["process"].update(fields)
        report["heartbeat_at"] = datetime.now(timezone.utc).isoformat()
        if "child_group" in fields:
            report["attempt_started"] = True
            report["actual_started_at"] = report["heartbeat_at"]
        write_json(report_path, report)

    write_json(report_path, report)
    code, interrupted = 1, 0
    cache = None
    try:
        report["source_start"] = source_identity(root)
        if plan:
            report["plan"] = plan
            if plan["source"] != report["source_start"]:
                raise ValueError("The verification plan is stale")
        if report["source_start"]["dirty"] and report["profile"] == "complete" and not plan:
            raise ValueError("Complete verification requires a clean tracked and untracked worktree")
        report["inventory"] = inventory(root)
        if not plan and command == ["make", "verify-local-steps"]:
            report["plan"] = complete_plan(root, base=context["base"] if context else "origin/main", with_graph=True)
            report["graph"] = report["plan"].pop("graph", None)
        if not plan and "graph" not in report:
            policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
            snapshot = {"source": report["source_start"], "checks": [],
                        "test_files": [f["path"] for f in report["inventory"]["files"]
                                       if f["kind"].endswith("-test")]}
            stage = command[command.index("step") + 1] if "step" in command else None
            if stage in set(policy.get("workflow", {}).get("profiles", {})) | set(policy.get("complete", {}).get("stages", [])):
                snapshot["checks"] = [{"group": stage, "files": [], "status": "ready", "reasons": ["Explicit public command"]}]
            verification_graph.attach(root, snapshot, policy, report["inventory"]["files"])
            report["graph"] = snapshot.get("graph")
        write_json(report_path, report)
        cache_started = time.monotonic()
        cache = verification_cache.DailyCache(root, plan, no_cache)
        report["cache"] = cache.observation
        report["budget"] = {"groups": 1, "workers": plan["workers"] if plan else "existing-complete-profile"}
        hit = cache.restore()
        report["cache_check_seconds"] = time.monotonic() - cache_started
        if hit:
            write_json(directory / "steps/cache.json", {"stage": "daily-result-reuse", "status": "cached",
                       "command": [], "duration_seconds": 0, "started_monotonic": time.monotonic()})
            code = 0
        else:
            cache.discard()
            code, interrupted = execute(command, {**verification_candidate.environment(),
                                                  "STORYOS_VERIFICATION_RUN": str(directory)},
                                        os.name == "posix", process_observation)
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
    if any(item["stage"] == "verification-tests" and item["command"] ==
           ["python3", "scripts/verification_test_files.py"] for item in steps):
        attempts = [json.loads(path.read_text()) for path in (directory / "nodes").glob("*.json")]
        report["verification_test_file_attempts"] = sorted(
            (item for item in attempts if item.get("node_id", "").startswith("file:verification-tools:")),
            key=lambda item: item["node_id"])
        expected = {"file:verification-tools:" + item["path"] for item in report["inventory"]["files"]
                    if item["kind"] == "verification-test" and item["group"] == "verification-tools"}
        actual = report["verification_test_file_attempts"]
        if (len(actual) != len(expected) or {item["node_id"] for item in actual} != expected
                or any(item["status"] != "passed" or not item["attempt_started"] for item in actual)):
            report["status"] = "incomplete"
    if (plan and code == 2 and report["status"] == "failed" and steps
            and all(item["status"] == "passed" for item in steps)
            and any(c.get("status") == "pending" for c in plan["checks"])):
        report["status"] = "pending"
    if (directory / "rust-test-files.json").is_file():
        report["rust_test_files"] = json.loads((directory / "rust-test-files.json").read_text())
    allowed = {"cached"} if cache and cache.observation["status"] == "hit" else {"passed"}
    if report["status"] == "passed" and (not steps or any(item["status"] not in allowed for item in steps)):
        report["status"] = "incomplete"
    if cache and report["status"] == "passed":
        try:
            cache_started = time.monotonic()
            cache.prepare(report_path)
            report["cache_output_check_seconds"] = time.monotonic() - cache_started
            report["source_end"] = source_identity(root)
            if report["source_end"] != report["source_start"]:
                report["status"] = "source-changed"
        except (OSError, ValueError) as error:
            report.update(status="failed", error=str(error))
    report["rust_cache_after"] = verification_rust_cache.finish(
        root, complete_success=report["status"] == "passed" and report["profile"] == "complete")
    report.update(duration_seconds=time.monotonic() - started, exit_code=code)
    report.update(ended_at=datetime.now(timezone.utc).isoformat(), ended_monotonic=time.monotonic())
    if report["process"].get("launch_error"):
        report["status"] = "infrastructure-failed"
    write_json(report_path, report)
    if cache:
        cache.publish(report_path)
    print(f"Verification {report['status']}: {report['duration_seconds']:.2f}s; report: {report_path}", flush=True)
    if report["status"] == "passed":
        return 0
    return 128 + interrupted if interrupted else (code if code > 0 else 1)


def run(root, command, *, plan=None, no_cache=False, context=None, locked=False):
    try:
        if not plan and command == ["make", "verify-local-steps"]:
            import verification_candidate
            return verification_candidate.run(root, command, context or {"base": "origin/main"})
        from contextlib import nullcontext
        with nullcontext() if locked else verification_cache.budget(root):
            rust_cache = verification_rust_cache.prepare(root)
            import verification_candidate
            context = {**(context or {}), "run_id": uuid.uuid4().hex}
            context["rust_cache"] = rust_cache
            verification_candidate.observe(root, "requested", emit=False, run_id=context["run_id"],
                                           issue=context.get("issue"), profile=context.get("profile", "daily"))
            return record_run(root, command, plan=plan, no_cache=no_cache, context=context)
    except (OSError, ValueError) as error:
        import verification_candidate
        verification_candidate.observe(root, "refused", emit=False, reason=str(error), issue=(context or {}).get("issue"))
        print(str(error), file=sys.stderr)
        return 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    commands.add_parser("rust-tests")
    commands.add_parser("inventory").add_argument("--check", action="store_true")
    for action in ("status", "targeted"):
        entry = commands.add_parser(action)
        entry.add_argument("--check", required=action == "targeted")
        if action == "status":
            entry.add_argument("--attempt")
        entry.add_argument("--json", action="store_true")
        entry.add_argument("--issue", type=int)
        entry.add_argument("--pr", type=int)
    commands.add_parser("recover-steps").add_argument("--attempt", required=True)
    recovery = commands.add_parser("recover")
    recovery.add_argument("--attempt", required=True)
    recovery.add_argument("--reason", required=True)
    for action in ("run", "step"):
        command_parser = commands.add_parser(action)
        if action == "step":
            command_parser.add_argument("stage")
            command_parser.add_argument("--node-id")
            command_parser.add_argument("--node-only", action="store_true")
        else:
            command_parser.add_argument("--base", default="origin/main")
            for name in ("issue", "pr"):
                command_parser.add_argument(f"--{name}", type=int)
            command_parser.add_argument("--review-request")
            command_parser.add_argument("--executor-context")
            command_parser.add_argument("--purpose", default="candidate")
            command_parser.add_argument("--trigger", default="explicit-request")
        command_parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    try:
        root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel"))
        if arguments.action in {"status", "targeted"}:
            if arguments.action == "targeted":
                return verification_status.execute(root, arguments.check, {"issue": arguments.issue, "pr": arguments.pr})
            verification_status.display(verification_status.attempt_status(root, arguments.attempt) if arguments.attempt
                                        else verification_status.status(root, verification_status.targeted_plan(root, arguments.check)), arguments.json)
            return 0
        if arguments.action == "rust-tests":
            if not os.environ.get("STORYOS_VERIFICATION_RUN"):
                return step(root, "rust-tests", [sys.executable, str(Path(__file__).resolve()), "rust-tests"])
            return rust_tests(root)
        if arguments.action == "inventory":
            result = inventory(root)
            print(f"Verified ownership of {len(result['files'])} input files" if arguments.check
                  else json.dumps(result, indent=2))
            return 0
        if arguments.action == "recover-steps":
            import verification_candidate
            return verification_candidate.recovery_steps(root, arguments.attempt)
        if arguments.action == "recover":
            import verification_candidate
            return verification_candidate.recover(root, arguments.attempt, arguments.reason)
        command = arguments.command
        if command[:1] == ["--"]:
            command = command[1:]
        if not command:
            raise ValueError("A verification command is required")
        if arguments.action == "run":
            return run(root, command, context={key: getattr(arguments, key) for key in
                       ("base", "issue", "pr", "purpose", "trigger", "review_request", "executor_context")})
        return step(root, arguments.stage, command, node_id=arguments.node_id, node_only=arguments.node_only)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
