#!/usr/bin/env python3
import json
from pathlib import Path
import sys


root = Path(__file__).resolve().parents[1]
metadata = json.load(sys.stdin)
manifests = sorted(Path(package["manifest_path"]).resolve() for package in metadata["packages"])
expected = [
    root / "crates/storyos-adapter-postgres/Cargo.toml",
    root / "crates/storyos-application/Cargo.toml",
    root / "crates/storyos-contracts/Cargo.toml",
    root / "crates/storyos-core/Cargo.toml",
    root / "crates/storyos-server/Cargo.toml",
    root / "crates/storyos-worker/Cargo.toml",
]
forbidden = [
    path for path in manifests if "/prototypes/" in str(path) or "/.reference/" in str(path)
]
if manifests != expected or forbidden:
    raise SystemExit(f"unexpected workspace manifests: {manifests}; forbidden: {forbidden}")
