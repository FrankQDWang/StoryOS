#!/usr/bin/env python3
"""Package one clean Git source tree with its exact Web bytes, its SkillPackages, and Server digest pin.

With --target linux-x86_64, the Rust build and the checks of the package run in Linux containers (ADR 0104
section 6). The Web resources and the manifest come from the same build of the host in each case.
"""

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


root = Path(__file__).resolve().parents[1]
target = root / "target"
binaries = ("storyos-server", "storyos-worker", "storyos-storage")
packages = ("-p", "storyos-server", "-p", "storyos-worker-bin", "-p", "storyos-adapter-postgres")
# The Rust version of Cargo.toml (rust-version) on Debian bookworm, which has glibc 2.36.
rust_image = "rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e"
# Seconds for the container build. A first build needs approximately 4 minutes (docs/operations/production-web.md).
build_limit = 3600


def output(*arguments):
    return subprocess.check_output(arguments, cwd=root, text=True).strip()


def source_identity():
    if output("git", "status", "--porcelain", "--untracked-files=all"):
        raise SystemExit("Release packaging requires a clean tracked and untracked worktree")
    return output("git", "rev-parse", "HEAD"), output("git", "rev-parse", "HEAD^{tree}")


def build_host(package, digest):
    build_target = target / "web-release"
    subprocess.run(["cargo", "build", "--locked", "--release", "--target-dir", str(build_target), *packages],
                   cwd=root, env={**os.environ, "STORYOS_WEB_MANIFEST_SHA256": digest}, check=True)
    suffix = ".exe" if os.name == "nt" else ""
    for binary in binaries:
        shutil.copy2(build_target / "release" / f"{binary}{suffix}", package / f"{binary}{suffix}")
    subprocess.run([str(package / f"storyos-server{suffix}"), "--check-web-root", str(package / "web")],
                   cwd=root, check=True)
    subprocess.run([str(package / f"storyos-worker{suffix}"), "--check"], cwd=root, check=True)


def build_linux(package, digest):
    # The Cargo home and the build output stay below target/, so a second build is incremental.
    build_root = target / "linux-x86_64"
    build_root.mkdir(exist_ok=True)
    container = f"storyos-linux-build-{os.getpid()}"
    try:
        subprocess.run(["docker", "run", "--rm", "--name", container, "--platform", "linux/amd64",
                        "--volume", f"{root}:/src:ro", "--volume", f"{build_root}:/src/target/linux-x86_64",
                        "--workdir", "/src", "--env", "CARGO_HOME=/src/target/linux-x86_64/cargo-home",
                        "--env", f"STORYOS_WEB_MANIFEST_SHA256={digest}", rust_image,
                        "cargo", "build", "--locked", "--release", "--target-dir",
                        "/src/target/linux-x86_64/build", *packages], check=True, timeout=build_limit)
    finally:
        subprocess.run(["docker", "rm", "-fv", container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for binary in binaries:
        shutil.copy2(build_root / "build" / "release" / binary, package / binary)
    subprocess.run([str(root / "scripts/check-linux-package.sh"), str(package)], cwd=root, check=True)


def write_archive(package, commit):
    name = f"storyos-linux-x86_64-{commit[:12]}"
    archive = target / f"{name}.tar.gz"
    checksum_file = target / f"{name}.tar.gz.sha256"
    # Both files are complete before they replace an earlier archive of the same commit.
    with tempfile.TemporaryDirectory(prefix="release-archive-", dir=target) as temporary:
        new_archive = Path(temporary) / archive.name
        with tarfile.open(new_archive, "w:gz") as tar:
            tar.add(package, arcname=name)
        checksum = hashlib.sha256(new_archive.read_bytes()).hexdigest()
        new_checksum_file = Path(temporary) / checksum_file.name
        new_checksum_file.write_text(f"{checksum}  {archive.name}\n")
        checksum_file.unlink(missing_ok=True)
        new_archive.replace(archive)
        new_checksum_file.replace(checksum_file)
    return archive, checksum


arguments = argparse.ArgumentParser(description=__doc__)
arguments.add_argument("--target", choices=("host", "linux-x86_64"), default="host")
selected = arguments.parse_args().target
source = source_identity()
subprocess.run(["pnpm", "--dir", "apps/web", "exec", "vite", "build"], cwd=root, check=True)
target.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix="release-package-", dir=target) as temporary:
    package = Path(temporary) / "package"
    shutil.copytree(root / "apps/web/dist", package / "web", symlinks=True)
    # Only the tracked files: an ignored file of the worktree stays out of the release package.
    for path in filter(None, output("git", "ls-files", "-z", "skill-packages").split("\0")):
        (package / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / path, package / path)
    digest = output("cargo", "run", "--quiet", "--locked", "-p", "storyos-contracts", "--",
                    "web-manifest", str(package / "web"), *source)
    if selected == "host":
        build_host(package, digest)
        destination = target / "release-package"
    else:
        build_linux(package, digest)
        destination = target / "release-package-linux-x86_64"
    if source_identity() != source:
        raise SystemExit("The Git source changed during release packaging")
    if destination.exists():
        shutil.rmtree(destination)
    package.rename(destination)
    print(f"Release package: {destination}\nSource: {source[0]}\nTree: {source[1]}\nManifest: {digest}")
    if selected == "linux-x86_64":
        archive, checksum = write_archive(destination, source[0])
        print(f"Archive: {archive}\nSHA-256: {checksum}")
