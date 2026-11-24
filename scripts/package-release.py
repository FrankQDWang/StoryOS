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
import signal
import subprocess
import tarfile
import tempfile


root = Path(__file__).resolve().parents[1]
target = root / "target"
binaries = ("storyos-server", "storyos-worker", "storyos-storage")
packages = ("-p", "storyos-server", "-p", "storyos-worker-bin", "-p", "storyos-adapter-postgres")
# The Rust version of Cargo.toml (rust-version) on Debian bookworm, which has glibc 2.36.
rust_image = "rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e"
# The distribution of the VPS of ADR 0022, which has glibc 2.39.
ubuntu_image = "ubuntu:24.04@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b"
# Seconds for each Docker command. A first build needs approximately 4 minutes (docs/operations/production-web.md),
# and each check of a correct package ends in a few seconds.
build_limit = 3600
check_limit = 120
cleanup_limit = 60


def output(*arguments):
    return subprocess.check_output(arguments, cwd=root, text=True).strip()


def run_limited(arguments, limit, capture=False, **options):
    """Run a command in a new process group, and stop the group when it does not end in `limit` seconds."""
    stdout = subprocess.PIPE if capture else None
    with subprocess.Popen(arguments, cwd=root, stdout=stdout, text=True, start_new_session=True,
                          **options) as process:
        try:
            result, _ = process.communicate(timeout=limit)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            raise SystemExit(f"{' '.join(arguments[:2])} did not end in {limit} s") from None
    if process.returncode != 0:
        raise SystemExit(f"{' '.join(arguments[:2])} failed with the status {process.returncode}")
    return result


def remove_containers(*names):
    subprocess.run(["docker", "rm", "-fv", *names], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                   timeout=cleanup_limit, check=False)


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
        run_limited(["docker", "run", "--rm", "--name", container, "--platform", "linux/amd64",
                     "--volume", f"{root}:/src:ro", "--volume", f"{build_root}:/src/target/linux-x86_64",
                     "--workdir", "/src", "--env", "CARGO_HOME=/src/target/linux-x86_64/cargo-home",
                     "--env", f"STORYOS_WEB_MANIFEST_SHA256={digest}", rust_image,
                     "cargo", "build", "--locked", "--release", "--target-dir",
                     "/src/target/linux-x86_64/build", *packages], build_limit)
    finally:
        remove_containers(container)
    for binary in binaries:
        shutil.copy2(build_root / "build" / "release" / binary, package / binary)
    check_linux_package(package)


def check_linux_package(package, limit=check_limit):
    """Check the package in an Ubuntu 24.04 container with no Rust: the Web root of the Server, the settings of the
    Worker, and storyos-storage against a new PostgreSQL container."""
    container = f"storyos-linux-package-{os.getpid()}"

    def in_ubuntu(name, *arguments):
        return run_limited(["docker", "run", "--rm", "--name", f"{container}-{name}", "--platform", "linux/amd64",
                            "--volume", f"{Path(package).resolve()}:/package:ro", *arguments], limit,
                           capture=True)

    try:
        print("Checking the Web root of the Server", flush=True)
        in_ubuntu("server", ubuntu_image, "/package/storyos-server", "--check-web-root", "/package/web")
        print("Checking the Worker", flush=True)
        in_ubuntu("worker", ubuntu_image, "/package/storyos-worker", "--check")
        print("Running storyos-storage against a new database", flush=True)
        run_limited(["sh", "-c", '. scripts/lib/controlled-postgres.sh && start_postgres "$0"', container],
                    limit, env={**os.environ, "repository_root": str(root)})
        activation = in_ubuntu("storage", "--network", f"container:{container}", "--env",
                               "STORYOS_STORAGE_ADMIN_URL=postgres://postgres:admin@127.0.0.1:5432/postgres",
                               ubuntu_image, "/package/storyos-storage").strip()
        print(activation)
        if activation != "STORYOS_STORAGE_ACTIVATION=Active":
            raise SystemExit("storyos-storage did not activate the database")
    finally:
        remove_containers(container, f"{container}-server", f"{container}-worker", f"{container}-storage")
    print("The Linux package passed its checks")


def write_archive(package, commit):
    name = f"storyos-linux-x86_64-{commit[:12]}"
    archive = target / f"{name}.tar.gz"
    checksum_file = target / f"{name}.tar.gz.sha256"
    # Both files are complete before they replace the files of an earlier build of the same commit. When the move of
    # the archive fails, the earlier pair stays as it was.
    with tempfile.TemporaryDirectory(prefix="release-archive-", dir=target) as temporary:
        new_archive = Path(temporary) / archive.name
        with tarfile.open(new_archive, "w:gz") as tar:
            tar.add(package, arcname=name)
        checksum = hashlib.sha256(new_archive.read_bytes()).hexdigest()
        new_checksum_file = Path(temporary) / checksum_file.name
        new_checksum_file.write_text(f"{checksum}  {archive.name}\n")
        new_archive.replace(archive)
        new_checksum_file.replace(checksum_file)
    return archive, checksum


def main():
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


if __name__ == "__main__":
    main()
