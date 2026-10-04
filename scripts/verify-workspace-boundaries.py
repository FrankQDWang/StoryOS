#!/usr/bin/env python3
import json
from pathlib import Path
import sys


root = Path(__file__).resolve().parents[1]
metadata = json.load(sys.stdin)
manifests = sorted(Path(package["manifest_path"]).resolve() for package in metadata["packages"])
expected = [
    root / "crates/storyos-adapter-fake-destination/Cargo.toml",
    root / "crates/storyos-adapter-postgres/Cargo.toml",
    root / "crates/storyos-application/Cargo.toml",
    root / "crates/storyos-contracts/Cargo.toml",
    root / "crates/storyos-core/Cargo.toml",
    root / "crates/storyos-server/Cargo.toml",
    root / "crates/storyos-worker/Cargo.toml",
    root / "crates/storyos-worker-bin/Cargo.toml",
]
forbidden = [
    path for path in manifests if "/prototypes/" in str(path) or "/.reference/" in str(path)
]
if manifests != expected or forbidden:
    raise SystemExit(f"unexpected workspace manifests: {manifests}; forbidden: {forbidden}")

# ADR 0039: Provider adapters stay out of the Server, the Worker library, and persistence.
dependencies = {
    package["name"]: {
        dependency["name"]
        for dependency in package["dependencies"]
        if dependency.get("kind") in (None, "build")
    }
    for package in metadata["packages"]
}


def closure(name):
    seen, pending = set(), [name]
    while pending:
        for dependency in dependencies.get(pending.pop(), ()):
            if dependency not in seen:
                seen.add(dependency)
                pending.append(dependency)
    return seen


problems = []
if "storyos-adapter-volcengine-responses" in closure("storyos-server"):
    problems.append("storyos-server links the Volcengine Responses adapter")
if any(name.startswith("storyos-adapter-") for name in dependencies["storyos-worker"]):
    problems.append("the storyos-worker library depends on an adapter")
if any("storyos-worker-bin" in names for names in dependencies.values()):
    problems.append("a package depends on the storyos-worker-bin composition root")
transport = {"reqwest", "hyper", "rustls", "native-tls", "openssl"}
if transport & closure("storyos-adapter-postgres"):
    problems.append("storyos-adapter-postgres depends on an HTTP or TLS client")
if problems:
    raise SystemExit("; ".join(problems))
