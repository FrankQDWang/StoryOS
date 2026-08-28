#!/usr/bin/env python3
"""Package one clean Git source tree with its exact Web bytes and Server digest pin."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile


root = Path(__file__).resolve().parents[1]


def output(*arguments):
    return subprocess.check_output(arguments, cwd=root, text=True).strip()


def source_identity():
    if output("git", "status", "--porcelain", "--untracked-files=all"):
        raise SystemExit("Release packaging requires a clean tracked and untracked worktree")
    return output("git", "rev-parse", "HEAD"), output("git", "rev-parse", "HEAD^{tree}")


source = source_identity()
subprocess.run(["pnpm", "--dir", "apps/web", "exec", "vite", "build"], cwd=root, check=True)
target = root / "target"
target.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix="release-package-", dir=target) as temporary:
    package = Path(temporary) / "package"
    shutil.copytree(root / "apps/web/dist", package / "web", symlinks=True)
    digest = output("cargo", "run", "--quiet", "--locked", "-p", "storyos-contracts", "--",
                    "web-manifest", str(package / "web"), *source)
    build_target = target / "web-release"
    subprocess.run(["cargo", "build", "--locked", "--release", "--target-dir", str(build_target),
                    "-p", "storyos-server"], cwd=root,
                   env={**os.environ, "STORYOS_WEB_MANIFEST_SHA256": digest}, check=True)
    executable = "storyos-server.exe" if os.name == "nt" else "storyos-server"
    shutil.copy2(build_target / "release" / executable, package / executable)
    if source_identity() != source:
        raise SystemExit("The Git source changed during release packaging")
    destination = target / "release-package"
    if destination.exists():
        shutil.rmtree(destination)
    package.rename(destination)
    print(f"Release package: {destination}\nSource: {source[0]}\nTree: {source[1]}\nManifest: {digest}")
