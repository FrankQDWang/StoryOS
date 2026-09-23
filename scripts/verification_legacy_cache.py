"""Retire the historical default Cargo cache through a sealed directory boundary."""

import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import uuid


TOP_LEVEL = {".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock",
             ".fingerprint", "build", "deps", "examples", "incremental"}
LOCKS = (".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock")
OUTPUT = re.compile(r"(?:libstoryos_[a-z_]+\.(?:d|rlib)|storyos-[a-z-]+(?:\.d)?)\Z")


def record_path(root):
    return root / "target/verification/legacy-rust-cache.json"


def write_record(root, record):
    path = record_path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, prefix=".legacy-rust-cache-",
                                         delete=False) as output:
            temporary = Path(output.name)
            output.write(json.dumps(record, indent=2) + "\n")
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def identity(path):
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or path.is_symlink():
        raise ValueError(f"Legacy Rust cache is not a directory: {path}")
    return {"device": info.st_dev, "inode": info.st_ino}


def inspect(path):
    from verification_rust_cache import measure

    if path.is_symlink() or not path.is_dir():
        raise ValueError("The historical Cargo debug path is not an owned directory")
    unknown = {child.name for child in path.iterdir() if child.name not in TOP_LEVEL
               and not OUTPUT.fullmatch(child.name)}
    if unknown:
        raise ValueError(f"Historical Cargo debug has unknown top-level content: {sorted(unknown)}")
    if not all(child.is_file() and not child.is_symlink() for child in path.iterdir()
               if OUTPUT.fullmatch(child.name)):
        raise ValueError("Historical Cargo output has an unexpected entry type")
    if not all((path / name).is_file() and not (path / name).is_symlink() for name in LOCKS):
        raise ValueError("Historical Cargo lock files are missing or linked")
    if not all((path / name).is_dir() and not (path / name).is_symlink()
               for name in (".fingerprint", "build", "deps", "incremental")):
        raise ValueError("Historical Cargo output directories are missing or linked")
    return measure(path)


def inspect_partial(path):
    from verification_rust_cache import measure

    unknown = {child.name for child in path.iterdir() if child.name not in TOP_LEVEL
               and not OUTPUT.fullmatch(child.name)}
    if unknown:
        raise ValueError(f"Quarantined Cargo output has unknown content: {sorted(unknown)}")
    if not all(child.is_file() and not child.is_symlink() for child in path.iterdir()
               if OUTPUT.fullmatch(child.name)):
        raise ValueError("Quarantined Cargo output has an unexpected entry type")
    for child in path.iterdir():
        if child.name in LOCKS and (not child.is_file() or child.is_symlink()):
            raise ValueError("Quarantined Cargo lock has an unexpected entry type")
        if child.name in TOP_LEVEL - set(LOCKS) and (not child.is_dir() or child.is_symlink()):
            raise ValueError("Quarantined Cargo directory has an unexpected entry type")
    return measure(path)


def references(path, lock_ids):
    """Return open references to a sealed tree, including unlinked Cargo lock inodes."""
    prefix = str(path.resolve()) + os.sep
    lock_ids = {(value["device"], value["inode"]) for value in lock_ids.values()}
    if sys.platform == "darwin":
        result = subprocess.run(["lsof", "-nP", "-FpcfDin"], capture_output=True, text=True)
        if result.returncode != 0 or result.stderr:
            raise ValueError("The open-file audit failed; keep the historical cache")
        pid = None
        device = inode = None
        found = []
        for line in result.stdout.splitlines():
            if line.startswith("p"):
                pid = int(line[1:])
            elif line.startswith("f"):
                device = inode = None
            elif line.startswith("D"):
                device = int(line[1:], 0)
            elif line.startswith("i") and line[1:].isdigit():
                inode = int(line[1:])
            elif line.startswith("n") and pid != os.getpid():
                name = line[1:]
                if name == str(path.resolve()) or name.startswith(prefix) or (device, inode) in lock_ids:
                    found.append({"pid": pid, "path": name})
        return found
    if sys.platform.startswith("linux"):
        found = []
        for process in Path("/proc").iterdir():
            if not process.name.isdigit() or int(process.name) == os.getpid():
                continue
            try:
                status = (process / "status").read_text()
                uid = next(line for line in status.splitlines() if line.startswith("Uid:"))
                if int(uid.split()[1]) != os.geteuid():
                    continue
                paths = [process / "cwd", process / "root", *(process / "fd").iterdir()]
                for descriptor in paths:
                    name = os.readlink(descriptor)
                    try:
                        info = descriptor.stat()
                    except FileNotFoundError:
                        continue
                    if name == str(path) or name.startswith(prefix) or (info.st_dev, info.st_ino) in lock_ids:
                        found.append({"pid": int(process.name), "path": name})
            except (FileNotFoundError, ProcessLookupError):
                continue
            except PermissionError as error:
                raise ValueError("The open-file audit lacks permission; keep the historical cache") from error
        return found
    raise ValueError("This host cannot audit open files for historical cache retirement")


def migrate(root):
    target = root / "target"
    cache_parent = target / "rust-cache"
    old = target / "debug"
    scratch = target / "issue-763-isolated"
    if target.is_symlink() or cache_parent.is_symlink() or old.is_symlink() or scratch.is_symlink():
        raise ValueError("A Rust cache migration path is linked")
    path = record_path(root)
    if path.is_symlink():
        raise ValueError("Historical Rust cache state path is linked")
    record = json.loads(path.read_text()) if path.exists() else None
    if record is None:
        if not old.exists():
            empty = {"logical_bytes": 0, "allocated_bytes": 0, "files": 0}
            record = {"version": 1, "state": "complete", "reason": "No historical default debug cache",
                      "before": empty, "removed_bytes": empty, "scratch_before": scratch_usage(scratch),
                      "scratch_removed_bytes": empty, "physical_reclaimed_bytes": None}
            if scratch.exists():
                scratch.rmdir()
            write_record(root, record)
            return record
        before = inspect(old)
        info = old.stat()
        locks = {name: identity_file(old / name) for name in LOCKS}
        quarantine = f"target/rust-cache/deleting-legacy-{uuid.uuid4().hex}"
        if (root / quarantine).exists():
            raise ValueError("Historical Rust cache quarantine path already exists")
        record = {"version": 1, "state": "planned", "path": quarantine,
                  "identity": identity(old), "mode": stat.S_IMODE(info.st_mode),
                  "locks": locks, "before": before, "scratch_before": scratch_usage(scratch)}
        write_record(root, record)
    if record.get("version") != 1 or record.get("state") not in ("planned", "quarantined", "complete"):
        raise ValueError("Unknown historical Rust cache migration state")
    if record["state"] == "complete":
        return record
    relative = record["path"]
    if not re.fullmatch(r"target/rust-cache/deleting-legacy-[0-9a-f]{32}", relative):
        raise ValueError("Historical Rust cache quarantine path is invalid")
    quarantine = root / relative
    old_owned = old.exists() and identity(old) == record["identity"]
    if quarantine.is_symlink() or (old_owned and quarantine.exists()):
        raise ValueError("Historical Rust cache migration has conflicting paths")
    if not old_owned and not quarantine.exists() and record["state"] == "quarantined":
        return complete(root, record, scratch)
    current = old if old_owned else quarantine
    if not current.exists() or identity(current) != record["identity"]:
        raise ValueError("Historical Rust cache directory identity changed")
    if current == old:
        record["before"] = inspect(old)
        current_locks = {name: identity_file(old / name) for name in LOCKS}
        if current_locks != record["locks"]:
            if references(old, record["locks"]):
                record["deferred"] = {"reason": "replaced Cargo lock remains open"}
                write_record(root, record)
                return record
            record["locks"] = current_locks
            write_record(root, record)
        busy = references(old, record["locks"])
        if busy:
            record["deferred"] = {"reason": "open Cargo path or lock", "references": busy[:8]}
            write_record(root, record)
            return record
        cache_parent.mkdir(parents=True, exist_ok=True)
        old.rename(quarantine)
        current = quarantine
    if record["state"] != "quarantined":
        record["state"] = "quarantined"
        write_record(root, record)
    if stat.S_IMODE(quarantine.stat().st_mode) not in (0, record["mode"]):
        raise ValueError("Historical Rust cache quarantine permissions changed")
    quarantine.chmod(0)
    busy = references(quarantine, record["locks"])
    if busy:
        record["deferred"] = {"reason": "open quarantined Cargo path or lock", "references": busy[:8]}
        write_record(root, record)
        return record
    quarantine.chmod(record["mode"])
    usage = inspect_partial(quarantine)
    if references(quarantine, record["locks"]):
        quarantine.chmod(0)
        raise ValueError("Historical Rust cache acquired an open reference after unsealing")
    if "retired_usage" not in record:
        record["retired_usage"] = usage
        write_record(root, record)
    shutil.rmtree(quarantine)
    return complete(root, record, scratch)


def complete(root, record, scratch):
    if scratch.exists():
        if not scratch.is_dir() or any(scratch.iterdir()):
            raise ValueError("Task scratch is not empty; keep it for review")
        scratch.rmdir()
    record.update(state="complete", reason="retire owned historical default Cargo cache",
                  removed_bytes=record["retired_usage"], scratch_removed_bytes=record["scratch_before"],
                  physical_reclaimed_bytes=None)
    record.pop("deferred", None)
    write_record(root, record)
    return record


def identity_file(path):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or path.is_symlink():
        raise ValueError("Historical Cargo lock is not a regular file")
    return {"device": info.st_dev, "inode": info.st_ino}


def scratch_usage(path):
    from verification_rust_cache import measure

    if not path.exists():
        return {"logical_bytes": 0, "allocated_bytes": 0, "files": 0}
    if path.is_symlink() or not path.is_dir() or any(path.iterdir()):
        raise ValueError("Task scratch has unknown content")
    return measure(path)
