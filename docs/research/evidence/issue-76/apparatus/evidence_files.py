"""Canonical evidence-bundle file inclusion rules."""

from __future__ import annotations

from pathlib import Path


def included_evidence_paths(out: Path) -> list[Path]:
    """Return every source or evidence file that the manifest must cover."""
    return sorted(
        path
        for path in out.rglob("*")
        if path.is_file()
        and path.name != "MANIFEST.sha256"
        and "node_modules" not in path.parts
        and "__pycache__" not in path.parts
    )
