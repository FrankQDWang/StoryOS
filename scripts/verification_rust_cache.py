#!/usr/bin/env python3
"""Own Cargo build generations used by repository verification commands."""

import argparse
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import uuid

import verification_cache


HIGH_WATER = 12 * 1024**3
TOTAL_LIMIT = 20 * 1024**3
TAG = "Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n# For information about cache directory tags see https://bford.info/cachedir/\n"
TOP_LEVEL = {".rustc_info.json", "CACHEDIR.TAG", ".storyos-rust-cache.json", "debug", "tmp"}


def limits(root):
    policy = root / "docs/agents/verification-policy.json"
    values = json.loads(policy.read_text()).get("rust_cache", {}) if policy.is_file() else {}
    high = values.get("high_water_bytes", HIGH_WATER)
    total = values.get("total_limit_bytes", TOTAL_LIMIT)
    if not all(isinstance(value, int) and not isinstance(value, bool) for value in (high, total)) or not 0 < high < total:
        raise ValueError("The Rust cache budget is invalid")
    return high, total


def state_path(root):
    return root / "target/verification/rust-cache.json"


def write_state(root, state):
    path = state_path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(state, indent=2) + "\n")
    temporary.replace(path)


def load(root):
    path = state_path(root)
    return json.loads(path.read_text()) if path.is_file() else None


def owned_path(root, entry, *, quarantine=False):
    relative = entry.get("quarantine" if quarantine else "path")
    if not isinstance(relative, str):
        raise ValueError("A Rust cache generation path is missing")
    path = root / relative
    allowed = (relative == "target/issue-763-workset" and not quarantine) or (
        relative == f"target/rust-cache/{'deleting' if quarantine else 'gen'}-{entry['id']}")
    target = root / "target"
    managed = root / "target/rust-cache"
    if (not allowed or not entry["id"].isalnum() or target.is_symlink() or path.is_symlink()
            or (relative.startswith("target/rust-cache/") and managed.is_symlink())
            or not path.resolve().is_relative_to(root.resolve() / "target")):
        raise ValueError("A Rust cache generation path is not owned")
    return path


def measure(path):
    logical = allocated = files = 0
    pending = [path]
    while pending:
        current = pending.pop()
        for child in os.scandir(current):
            info = child.stat(follow_symlinks=False)
            if stat.S_ISLNK(info.st_mode):
                raise ValueError(f"Rust cache contains a link: {child.path}")
            if stat.S_ISDIR(info.st_mode):
                pending.append(Path(child.path))
            elif stat.S_ISREG(info.st_mode):
                logical += info.st_size
                allocated += info.st_blocks * 512
                files += 1
            else:
                raise ValueError(f"Rust cache contains an unsupported entry: {child.path}")
    return {"logical_bytes": logical, "allocated_bytes": allocated, "files": files}


def validate(root, entry):
    path = owned_path(root, entry)
    if not path.is_dir():
        raise ValueError(f"Rust cache generation is missing: {path}")
    marker = path / ".storyos-rust-cache.json"
    if (not marker.is_file() or marker.is_symlink()
            or json.loads(marker.read_text()) != {"version": 1, "id": entry["id"]}):
        raise ValueError(f"Rust cache generation owner marker is invalid: {path}")
    if {child.name for child in path.iterdir()} - TOP_LEVEL:
        raise ValueError(f"Rust cache generation has unknown top-level content: {path}")
    return measure(path)


def reserve():
    identifier = uuid.uuid4().hex
    return {"id": identifier, "path": f"target/rust-cache/gen-{identifier}", "warmup": True}


def create(root, entry):
    path = owned_path(root, entry)
    if not path.exists():
        path.mkdir(parents=True)
    allowed = {".storyos-rust-cache.json", ".storyos-rust-cache.tmp", "CACHEDIR.TAG"}
    if {child.name for child in path.iterdir()} - allowed:
        raise ValueError("Pending Rust cache generation has unknown content")
    measure(path)
    marker = path / ".storyos-rust-cache.json"
    if marker.is_file():
        try:
            owner = json.loads(marker.read_text())
        except ValueError:
            owner = None
        if owner is not None and owner != {"version": 1, "id": entry["id"]}:
            raise ValueError("Pending Rust cache owner marker conflicts with state")
    temporary = path / ".storyos-rust-cache.tmp"
    temporary.write_text(json.dumps({"version": 1, "id": entry["id"]}) + "\n")
    temporary.replace(marker)
    (path / "CACHEDIR.TAG").write_text(TAG)


def retire(root, state):
    for entry in list(state["retired"]):
        if entry["id"] == state["active"]["id"]:
            raise ValueError("The active Rust cache generation cannot be retired")
        original = owned_path(root, entry)
        quarantine = owned_path(root, entry, quarantine=True) if entry.get("quarantine") else None
        if quarantine is None:
            validate(root, entry)
            entry["quarantine"] = f"target/rust-cache/deleting-{entry['id']}"
            quarantine = owned_path(root, entry, quarantine=True)
            write_state(root, state)
        if original.exists() and not quarantine.exists():
            validate(root, entry)
            quarantine.parent.mkdir(parents=True, exist_ok=True)
            original.rename(quarantine)
        if original.exists() or (quarantine.exists() and quarantine.is_symlink()):
            raise ValueError("Rust cache retirement has conflicting paths")
        if quarantine.exists():
            if {child.name for child in quarantine.iterdir()} - TOP_LEVEL:
                raise ValueError("Retired Rust cache has unknown top-level content")
            marker = quarantine / ".storyos-rust-cache.json"
            if marker.exists():
                if marker.is_symlink() or json.loads(marker.read_text()) != {"version": 1, "id": entry["id"]}:
                    raise ValueError("Retired Rust cache owner marker is invalid")
                measure(quarantine)
                for child in quarantine.iterdir():
                    if child != marker:
                        if child.is_dir():
                            shutil.rmtree(child)
                        else:
                            child.unlink()
                marker.unlink()
            elif any(quarantine.iterdir()):
                raise ValueError("Retired Rust cache has content without its owner marker")
            quarantine.rmdir()
        state["retired"].remove(entry)
        state["events"].append({"reason": "high-water retirement", "id": entry["id"],
                                 "removed_bytes": entry.get("bytes_before"),
                                 "physical_reclaimed_bytes": None})
        state["events"] = state["events"][-20:]
        write_state(root, state)


def prepare(root):
    high_water, total_limit = limits(root)
    inherited = os.environ.get("STORYOS_RUST_CACHE_ROOT")
    if inherited and Path(inherited).resolve() != root.resolve():
        os.environ.pop("CARGO_TARGET_DIR", None)
    state = load(root)
    if state is None:
        if os.environ.get("CARGO_TARGET_DIR"):
            raise ValueError("CARGO_TARGET_DIR conflicts with the managed Rust cache generation")
        workset = root / "target/issue-763-workset"
        if workset.is_symlink():
            raise ValueError("The measured workset is a link")
        if workset.exists():
            owner = json.loads((workset / ".storyos-rust-cache.json").read_text())
            active = {"id": owner["id"], "path": "target/issue-763-workset", "warmup": False}
            validate(root, active)
        else:
            active = reserve()
        state = {"version": 1, "active": active, "retired": [], "events": [],
                 "pending_create": not workset.exists()}
        write_state(root, state)
    if state.get("version") != 1:
        raise ValueError("Unknown Rust cache state version")
    requested = os.environ.get("CARGO_TARGET_DIR")
    if requested and Path(requested).resolve() != owned_path(root, state["active"]).resolve():
        raise ValueError("CARGO_TARGET_DIR conflicts with the managed Rust cache generation")
    if state.get("pending_create"):
        create(root, state["active"])
        state["pending_create"] = False
        write_state(root, state)
    retire(root, state)
    usage = validate(root, state["active"])
    scratch = root / "target/issue-763-isolated"
    scratch_usage = measure(scratch) if scratch.is_dir() and not scratch.is_symlink() else {
        "logical_bytes": 0, "allocated_bytes": 0, "files": 0}
    if scratch.is_symlink() or usage["allocated_bytes"] + scratch_usage["allocated_bytes"] > total_limit:
        raise ValueError("The Rust build set and task scratch exceed the managed limit; review the cache budget")
    if usage["allocated_bytes"] > high_water and not state["active"]["warmup"]:
        old = state["active"]
        old["bytes_before"] = usage
        state["retired"].append(old)
        state["active"] = reserve()
        state["pending_create"] = True
        write_state(root, state)
        create(root, state["active"])
        state["pending_create"] = False
        write_state(root, state)
        retire(root, state)
        usage = validate(root, state["active"])
    target = owned_path(root, state["active"])
    os.environ["CARGO_TARGET_DIR"] = str(target)
    os.environ["STORYOS_RUST_CACHE_ROOT"] = str(root)
    return {"generation": state["active"]["id"], "target_dir": str(target), "profile": "dev",
            "warmup": state["active"]["warmup"], "usage": usage, "scratch_usage": scratch_usage,
            "high_water_bytes": high_water, "total_limit_bytes": total_limit,
            "state": "over-budget" if usage["allocated_bytes"] > high_water else "ready"}


def finish(root, *, complete_success=False):
    high_water, total_limit = limits(root)
    state = load(root)
    usage = validate(root, state["active"])
    if complete_success:
        state["active"]["warmup"] = usage["allocated_bytes"] > high_water
        write_state(root, state)
    return {"generation": state["active"]["id"], "usage": usage,
            "state": "over-limit" if usage["allocated_bytes"] > total_limit else (
                "over-budget" if usage["allocated_bytes"] > high_water else "ready")}


def identity(root):
    state = load(root)
    if state is None:
        return {"generation": None, "target_dir": None, "profile": "dev"}
    target = owned_path(root, state["active"])
    return {"generation": state["active"]["id"], "target_dir": str(target), "profile": "dev"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("action", choices=("status", "run"))
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        root = args.root.resolve()
        with verification_cache.budget(root):
            before = prepare(root)
            if args.action == "status":
                print(json.dumps(before, indent=2))
                return 0
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            if not command:
                raise ValueError("A managed command is required")
            result = subprocess.run(command, cwd=root)
            finish(root)
            return result.returncode
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
