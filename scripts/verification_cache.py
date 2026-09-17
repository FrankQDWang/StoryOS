"""Reuse complete isolated daily results within the local execution trust boundary."""

from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


@contextmanager
def budget(root):
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


def outputs(root, producer):
    directories = [root / name for name in ("node_modules", "apps/web/node_modules")]
    if not all(path.is_dir() for path in directories) or not (producer / "vitest.json").is_file():
        return None
    identities = []
    for directory in directories:
        for path in sorted(directory.rglob("*")):
            if path.is_symlink():
                if (path.resolve() != (root / "apps/web").resolve()
                        and not any(path.resolve().is_relative_to(base.resolve()) for base in directories)):
                    return None
                identities.append((str(path.relative_to(root)), "link", os.readlink(path)))
            elif path.is_file():
                identities.append((str(path.relative_to(root)), "file", hashlib.sha256(path.read_bytes()).hexdigest()))
    identities.append(("test-result", hashlib.sha256((producer / "vitest.json").read_bytes()).hexdigest()))
    return digest(identities)


class DailyCache:
    def __init__(self, root, plan, no_cache):
        self.root, self.path, self.no_cache, self.required = root, None, no_cache, None
        self.observation = {"status": "disabled", "reason": "The selected group has no reviewed cache profile"}
        policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
        if (not plan or "node-contract" not in policy.get("result_cache_profiles", [])
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
                           if key not in {"STORYOS_VERIFICATION_RUN", "STORYOS_VERIFICATION_PARENT", "_", "SHLVL"}}
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
        elif self.observation["status"] == "hit":
            producer = self.root / self.observation["producer"]
            if (self.required != outputs(self.root, producer.parent)
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
