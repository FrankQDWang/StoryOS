"""Reuse complete isolated daily results within the local execution trust boundary."""

from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import threading
import time
import uuid

import verification_failure

QUEUE_HELD = "STORYOS_VERIFICATION_HOST_QUEUE"


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


@contextmanager
def budget(root):
    if (root / "target").is_symlink() or (root / "target/verification").is_symlink():
        raise ValueError("The verification budget path must remain inside the repository")
    directory = root / "target/verification"
    directory.mkdir(parents=True, exist_ok=True)
    with (directory / "host.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("The repository verification budget is busy; retry after the active run") from error
        try:
            yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def queue_path(root):
    common = subprocess.check_output(["git", "rev-parse", "--git-common-dir"], cwd=root, text=True).strip()
    return (Path(root) / common).resolve() / "storyos-host-queue.lock"


def holder_alive(holder):
    try:
        os.kill(int(holder["pid"]), 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except (KeyError, TypeError, ValueError, OverflowError):
        return False
    return True


def read_holder(handle):
    handle.seek(0)
    try:
        holder = json.loads(handle.read() or "null")
    except ValueError:
        return None
    return holder if isinstance(holder, dict) else None


def queue_state(root):
    """Return the holder of the host queue, or "free" when no live process holds it."""
    try:
        with queue_path(root).open() as handle:
            holder = read_holder(handle)
    except FileNotFoundError:
        return "free"
    return holder if holder and holder_alive(holder) else "free"


def describe(holder):
    return (f"stage {holder.get('stage')} of worktree {holder.get('worktree')}, "
            f"started {holder.get('started_at')}, process {holder.get('pid')}")


@contextmanager
def host_queue(root, stage, *, report_seconds=120.0):
    """Hold the host queue that all worktrees of the repository share, and wait while another run holds it."""
    if os.environ.get(QUEUE_HELD):
        yield None
        return
    path = queue_path(root)
    record = {"stage": stage, "lock": str(path), "waited_seconds": 0.0, "released": None}
    started = time.monotonic()
    handle = path.open("a+")
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        done = threading.Event()

        def report():
            while True:
                print(f"Host queue: waiting {time.monotonic() - started:.0f}s for "
                      f"{describe(read_holder(handle) or {})}", flush=True)
                if done.wait(report_seconds):
                    return

        reporter = threading.Thread(target=report, daemon=True)
        reporter.start()
        try:
            fcntl.flock(handle, fcntl.LOCK_EX)
        finally:
            done.set()
            reporter.join()
    previous = read_holder(handle)
    if previous:
        record["released"] = previous
        print(f"Host queue: released the lock of {describe(previous)}; that process ended without a release",
              flush=True)
    holder = {"pid": os.getpid(), "worktree": str(Path(root).resolve()), "stage": stage,
              "started_at": datetime.now(timezone.utc).isoformat()}
    handle.seek(0)
    handle.truncate()
    handle.write(json.dumps(holder))
    handle.flush()
    record.update(holder=holder, waited_seconds=round(time.monotonic() - started, 3))
    run_path = os.environ.get("STORYOS_VERIFICATION_RUN")
    if run_path:
        directory = Path(run_path) / "host-queue"
        directory.mkdir(exist_ok=True)
        (directory / f"{uuid.uuid4().hex}.json").write_text(json.dumps(record))
    os.environ[QUEUE_HELD] = str(path)
    try:
        yield record
    finally:
        os.environ.pop(QUEUE_HELD, None)
        handle.seek(0)
        handle.truncate()
        handle.flush()
        fcntl.flock(handle, fcntl.LOCK_UN)
        handle.close()


def scan(root):
    """Return the identity of each present dependency file and whether the installation is reusable."""
    directories = [root / name for name in ("node_modules", "apps/web/node_modules")]
    reusable = all(path.is_dir() for path in directories)
    ignored = set(json.loads((root / "docs/agents/verification-policy.json").read_text()).get("dependency_ignore", []))
    identities = []
    for directory in directories:
        for path in sorted(directory.rglob("*")):
            if ignored.intersection(path.relative_to(directory).parts):
                continue
            if path.is_symlink():
                if (path.resolve() != (root / "apps/web").resolve()
                        and not any(path.resolve().is_relative_to(base.resolve()) for base in directories)):
                    reusable = False
                content = ("link", os.readlink(path))
            elif path.is_file():
                content = hashlib.sha256(path.read_bytes()).hexdigest()
            else:
                continue
            state = path.lstat()
            identities.append((str(path.relative_to(root)), state.st_mode, state.st_ino,
                               state.st_mtime_ns, state.st_ctime_ns, content))
    return identities, reusable


def installed(root):
    """Return the identity of each installed dependency file, or None when the installation is not reusable."""
    identities, reusable = scan(root)
    return identities if reusable else None


def changed_dependencies(before, root):
    """Return the sorted dependency paths whose identity differs from an earlier installed() result."""
    def index(entries):
        return {entry[0]: json.dumps(entry) for entry in entries or []}
    return verification_failure.changed(index(before), index(scan(root)[0]))


def outputs(root, producer=None):
    identities = installed(root)
    if identities is None:
        return None
    if producer:
        return {"dependencies": digest(identities), "artifacts": {
            name: hashlib.sha256((producer / name).read_bytes()).hexdigest()
            for name in ("vitest.json", "dependencies.json")}}
    return digest(identities)


TOOL_RESULTS = "target/verification-cache/verification-tests"
DIAGNOSTIC = ("STORYOS_VERIFICATION_TEST_WORKERS", "STORYOS_VERIFICATION_COMPARE")


def tool_key(root):
    """Return the digest of the inputs that the verification-tool self-tests read."""
    import verification
    contents = []
    for path in verification.input_paths(root):
        if path == "Makefile" or path.startswith(("scripts/", "docs/agents/", ".github/")):
            source = root / path
            contents.append((path, source.lstat().st_mode, os.readlink(source) if source.is_symlink() else None,
                             hashlib.sha256(source.read_bytes()).hexdigest() if source.is_file() else None)
                            if source.is_file() or source.is_symlink() else (path, None))
    tests = sorted((item["path"], item["kind"], item["group"]) for item in verification.inventory(root)["files"]
                   if item["kind"].endswith("-test"))
    node = shutil.which("node")
    versions = [sys.version, subprocess.check_output([node, "--version"], text=True).strip() if node else None]
    return digest({"version": 1, "inputs": contents, "tests": tests, "versions": versions})


def tool_result(root, key):
    """Return the producer of a passed verification-tool self-test result with this key, or None."""
    try:
        entry = json.loads((root / TOOL_RESULTS / f"{key}.json").read_text())
        producer = (root / entry["report"]).resolve()
        if not producer.is_relative_to((root / "target/verification").resolve()):
            return None
        report = json.loads(producer.read_text())
        step = next(item for item in report["steps"] if item.get("id") == entry["step"])
        if (step["stage"] != "verification-tests" or step["status"] != "passed" or step.get("reuse_key") != key
                or report["source_start"] != report["source_end"]
                or any(item["status"] != "passed" for item in report.get("verification_test_file_attempts", []))):
            return None
        return {"producer": entry["report"], "producer_step": step["id"]}
    except (OSError, ValueError, KeyError, TypeError, StopIteration):
        return None


def publish_tool_result(root, report_path, step):
    """Record a passed verification-tool self-test step as the reusable result of its key."""
    path = root / TOOL_RESULTS / f"{step['reuse_key']}.json"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(".tmp")
        temporary.write_text(json.dumps({"key": step["reuse_key"], "report": str(report_path.relative_to(root)),
                                         "step": step["id"]}, indent=2) + "\n")
        temporary.replace(path)
    except OSError:
        pass


class DailyCache:
    def __init__(self, root, plan, no_cache):
        self.root, self.path, self.no_cache, self.required = root, None, no_cache, None
        self.observation = {"status": "disabled", "reason": "The selected group has no reviewed cache profile"}
        policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
        if (not plan or any(c.get("requires_package") for c in plan["checks"])
                or "node-contract" not in policy.get("result_cache_profiles", [])
                or [check["group"] for check in plan["checks"]] != ["policy", "web-typecheck", "node-contract"]):
            return
        try:
            tools = []
            for name in ("git", "make", "cargo", "rustc", "node", "pnpm"):
                executable = Path(shutil.which(name) or "").resolve()
                tools.append((name, str(executable), hashlib.sha256(executable.read_bytes()).hexdigest(),
                              subprocess.check_output([name, "--version"], cwd=root, text=True,
                                                      stderr=subprocess.PIPE)))
            environment = {key: value for key, value in os.environ.items()
                           if key not in {"STORYOS_VERIFICATION_RUN", "STORYOS_VERIFICATION_PARENT",
                                          "STORYOS_RUST_CACHE_ROOT", "_", "SHLVL"}}
            runners = [(path.name, hashlib.sha256(path.read_bytes()).hexdigest())
                       for path in sorted(Path(__file__).parent.glob("verification*.py"))]
            key = digest({"version": 1, "inputs": plan["source"]["inputs_sha256"],
                          "checks": plan["checks"], "membership": plan["test_files"], "workers": plan["workers"],
                          "tools": tools, "environment": digest(environment), "runners": runners,
                          "host": (platform.node(), platform.platform(), platform.machine(), sys.version, sys.executable)})
            self.path = root / "target/verification-cache" / f"{key}.json"
            self.observation = {"status": "bypass" if no_cache else "miss", "key": key}
        except (OSError, subprocess.CalledProcessError) as error:
            self.observation["reason"] = f"Toolchain identity is unavailable: {type(error).__name__}"

    def restore(self):
        if self.path is None or self.no_cache:
            return False
        try:
            entry = json.loads(self.path.read_text())
            producer = (self.root / entry["report"]).resolve()
            if not producer.is_relative_to((self.root / "target/verification").resolve()):
                return False
            data = producer.read_bytes()
            report = json.loads(data)
            if (entry["key"] != self.observation["key"] or entry["report_sha256"] != hashlib.sha256(data).hexdigest()
                    or report["status"] != "passed" or report["profile"] != "daily"
                    or report["source_start"] != report["source_end"] or not report["steps"]
                    or any(step["status"] != "passed" for step in report["steps"])
                    or report["cache"] != self.observation or not entry["outputs"]
                    or entry["outputs"] != outputs(self.root, producer.parent)):
                return False
            self.observation.update(status="hit", producer=entry["report"], report_sha256=entry["report_sha256"])
            self.required = entry["outputs"]
            return True
        except (OSError, ValueError, KeyError, TypeError):
            return False

    def discard(self):
        if self.path:
            self.path.unlink(missing_ok=True)

    def prepare(self, report_path):
        if self.path is not None and not self.no_cache and self.observation["status"] == "miss":
            self.required = outputs(self.root, report_path.parent)
            if (self.required or {}).get("dependencies") != json.loads(
                    (report_path.parent / "dependencies.json").read_text()):
                reason = "Installed dependencies changed before cache publication"
                (report_path.parent / "failure.json").write_text(json.dumps({
                    "reason": reason, "changed_paths": changed_dependencies(
                        json.loads((report_path.parent / "installed.json").read_text()), self.root)}))
                raise ValueError(reason)
        elif self.observation["status"] == "hit":
            producer = self.root / self.observation["producer"]
            current = outputs(self.root, producer.parent)
            recorded = producer.parent / "installed.json"
            if (current or {}).get("dependencies") != self.required["dependencies"] and recorded.is_file():
                reason = "Installed dependencies changed during cache reuse"
                (report_path.parent / "failure.json").write_text(json.dumps({
                    "reason": reason,
                    "changed_paths": changed_dependencies(json.loads(recorded.read_text()), self.root)}))
                raise ValueError(reason)
            if (self.required != current
                    or self.observation["report_sha256"] != hashlib.sha256(producer.read_bytes()).hexdigest()):
                raise ValueError("Cached verification outputs changed during reuse")

    def publish(self, report_path):
        if self.path is None or self.no_cache or self.observation["status"] != "miss":
            return
        try:
            data = report_path.read_bytes()
            report = json.loads(data)
            if report["status"] != "passed" or not self.required:
                return
            entry = {"key": self.observation["key"], "report": str(report_path.relative_to(self.root)),
                     "report_sha256": hashlib.sha256(data).hexdigest(), "outputs": self.required}
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary = self.path.with_suffix(".tmp")
            temporary.write_text(json.dumps(entry, indent=2) + "\n")
            temporary.replace(self.path)
        except OSError:
            pass
