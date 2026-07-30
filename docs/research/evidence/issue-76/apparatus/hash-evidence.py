#!/usr/bin/env python3
"""Write SHA-256 identities for the frozen Issue #76 evidence bundle."""

from __future__ import annotations

import hashlib
from pathlib import Path

from evidence_files import included_evidence_paths


HERE = Path(__file__).resolve().parent
OUT = HERE.parent
paths = included_evidence_paths(OUT)
lines = [
    f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(OUT)}"
    for path in paths
]
(OUT / "MANIFEST.sha256").write_text("\n".join(lines) + "\n", encoding="utf-8")
