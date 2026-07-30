#!/usr/bin/env python3
"""Correct only the deterministic journal comparator in frozen browser evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


HERE = Path(__file__).resolve().parent
OUT = HERE.parent
RAW = OUT / "browser-measurements.jsonl"
PROVENANCE = OUT / "browser-evidence-correction.json"
RAW_REPO_PATH = (
    "docs/research/evidence/issue-76/browser-measurements.jsonl"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-main", required=True)
    args = parser.parse_args()

    original_bytes = subprocess.run(
        ["git", "show", f"{args.source_main}:{RAW_REPO_PATH}"],
        check=True,
        capture_output=True,
    ).stdout
    original_lines = original_bytes.splitlines()
    parsed = [json.loads(line) for line in original_lines]
    target_indexes = [
        index for index, row in enumerate(parsed) if row["kind"] == "journal_growth"
    ]
    if len(target_indexes) != 1:
        raise ValueError(
            f"expected one journal_growth row, got {len(target_indexes)}"
        )

    target_index = target_indexes[0]
    row = parsed[target_index]
    intents = int(row["intent_count"])
    patch_bytes = int(row["patch_utf8_bytes_actual"])
    observed = int(row["naive_before_after_full_copy_bytes"])
    known_defective = patch_bytes * intents * (intents + 1)
    corrected = patch_bytes * intents**2
    if observed != known_defective:
        raise ValueError(
            "frozen source does not contain the known twice-post-edit comparator: "
            f"observed={observed} expected={known_defective}"
        )

    unaffected_bytes = b"\n".join(
        line for index, line in enumerate(original_lines) if index != target_index
    ) + b"\n"
    row["naive_before_after_full_copy_bytes"] = corrected
    corrected_lines = list(original_lines)
    corrected_lines[target_index] = json.dumps(
        row, separators=(",", ":")
    ).encode("utf-8")
    corrected_bytes = b"\n".join(corrected_lines) + b"\n"
    RAW.write_bytes(corrected_bytes)

    provenance = {
        "schema": "storyos.issue76.browser_evidence_correction.v1",
        "source_main_commit": args.source_main,
        "source_raw_sha256": sha256(original_bytes),
        "corrected_raw_sha256": sha256(corrected_bytes),
        "target_zero_based_line": target_index,
        "unchanged_row_count": len(original_lines) - 1,
        "unaffected_rows_sha256": sha256(unaffected_bytes),
        "intent_count": intents,
        "patch_utf8_bytes_actual": patch_bytes,
        "previous_twice_post_edit_bytes": observed,
        "corrected_before_plus_after_bytes": corrected,
        "formula": (
            "sum_i(((i - 1) * patch_utf8_bytes_actual) "
            "+ (i * patch_utf8_bytes_actual))"
        ),
    }
    PROVENANCE.write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(provenance, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
