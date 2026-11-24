#!/usr/bin/env python3
import json
from pathlib import Path
import sys


root = Path(__file__).resolve().parents[1]
metadata = json.load(sys.stdin)
manifests = sorted(Path(package["manifest_path"]).resolve() for package in metadata["packages"])
expected = [
    root / "crates/storyos-adapter-diagnostics/Cargo.toml",
    root / "crates/storyos-adapter-fake-destination/Cargo.toml",
    root / "crates/storyos-adapter-mcp-client/Cargo.toml",
    root / "crates/storyos-adapter-postgres/Cargo.toml",
    root / "crates/storyos-adapter-volcengine-responses/Cargo.toml",
    root / "crates/storyos-application/Cargo.toml",
    root / "crates/storyos-contracts/Cargo.toml",
    root / "crates/storyos-core/Cargo.toml",
    root / "crates/storyos-literal-guard/Cargo.toml",
    root / "crates/storyos-server/Cargo.toml",
    root / "crates/storyos-settings/Cargo.toml",
    root / "crates/storyos-worker/Cargo.toml",
    root / "crates/storyos-worker-bin/Cargo.toml",
]
forbidden = [path for path in manifests if "/.reference/" in str(path)]
if manifests != expected or forbidden:
    raise SystemExit(f"unexpected workspace manifests: {manifests}; forbidden: {forbidden}")

# ADR 0039: a Model Provider Adapter for a real Provider stays out of the Server, the Worker
# library, and persistence. The Server may use the Contract-Faithful Fake Destination adapter.
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


# A tool crate checks the repository. It stays out of every product crate and the release package.
tool_crates = {"storyos-literal-guard"}

problems = []
for adapter in ("storyos-adapter-volcengine-responses", "storyos-adapter-mcp-client"):
    if adapter in closure("storyos-server"):
        problems.append(f"storyos-server links {adapter}")
if any(name.startswith("storyos-adapter-") for name in dependencies["storyos-worker"]):
    problems.append("the storyos-worker library depends on an adapter")
if any("storyos-worker-bin" in names for names in dependencies.values()):
    problems.append("a package depends on the storyos-worker-bin composition root")
if any(tool_crates & names for names in dependencies.values()):
    problems.append("a package depends on a repository tool crate")
# ADR 0104 section 5: the adapter has the rustls TLS of the PostgreSQL connection, and no HTTP client and no
# other TLS stack.
transport = {"reqwest", "hyper", "native-tls", "openssl"}
if transport & closure("storyos-adapter-postgres"):
    problems.append("storyos-adapter-postgres depends on an HTTP client or another TLS stack")
# ADR 0089: only the Server decodes the YAML frontmatter of a SkillPackage.
for name in dependencies:
    if name != "storyos-server" and "yaml-rust2" in closure(name):
        problems.append(f"{name} links yaml-rust2")
if problems:
    raise SystemExit("; ".join(problems))
