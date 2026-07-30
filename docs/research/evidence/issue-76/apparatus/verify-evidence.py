#!/usr/bin/env python3
"""Non-mutating integrity and invariant checks for the Issue #76 bundle."""

from __future__ import annotations

import csv
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
OUT = HERE.parent


def json_lines(path: Path) -> list[dict[str, object]]:
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line
    ]


workload = json.loads((HERE / "workload.json").read_text(encoding="utf-8"))
browser = json_lines(OUT / "browser-measurements.jsonl")
operations = json_lines(OUT / "postgres-operations.jsonl")
summary = json.loads((OUT / "summary.json").read_text(encoding="utf-8"))
environment = json.loads((OUT / "environment.json").read_text(encoding="utf-8"))
with (OUT / "postgres-relation-growth.csv").open(
    encoding="utf-8", newline=""
) as handle:
    relations = list(csv.DictReader(handle))

assert workload["schema"] == "storyos.issue76.workload.v1"
assert len(browser) == 422
assert len(relations) == 468
assert len(operations) == 192
assert summary["schema"] == "storyos.issue76.summary.v1"
assert environment["schema"] == "storyos.issue76.environment.v1"
assert environment["workload_sha256"] == hashlib.sha256(
    (HERE / "workload.json").read_bytes()
).hexdigest()
assert all(row["schema"] == "storyos.issue76.browser.v1" for row in browser)
assert all(
    row["schema"] == "storyos.issue76.postgres_operation.v1"
    for row in operations
)
assert {row["sample"] for row in operations} == {0, 1, 2}
assert {
    row["profile"]
    for row in operations
} == {
    "local_4vcpu_4gib",
    "controlled_cloud_surrogate_2vcpu_2gib",
}

restore_rows = [
    row for row in operations if row["operation"] == "logical_backup_restore"
]
assert len(restore_rows) == 6
assert all(row["validated_equal"] is True for row in restore_rows)
assert all(
    row["source_count_and_sequence_sum"]
    == row["restored_count_and_sequence_sum"]
    for row in restore_rows
)

synthetic_ime = [
    row for row in browser if row["kind"] == "synthetic_composition"
]
assert len(synthetic_ime) == workload["browser"]["measured_samples"]
assert all(
    row["language_probe"] == "synthetic_chinese_composition_not_os_ime"
    for row in synthetic_ime
)
environment_rows = [
    row for row in browser if row["kind"] == "browser_environment"
]
assert len(environment_rows) == 1
assert environment_rows[0]["event_timing_entries"] == []

manifest_entries = {}
for line in (OUT / "MANIFEST.sha256").read_text(encoding="utf-8").splitlines():
    digest, relative = line.split("  ", 1)
    manifest_entries[relative] = digest
assert all(".reference" not in relative for relative in manifest_entries)
for relative, digest in manifest_entries.items():
    assert hashlib.sha256((OUT / relative).read_bytes()).hexdigest() == digest

print(
    json.dumps(
        {
            "browser_rows": len(browser),
            "postgres_relation_rows": len(relations),
            "postgres_operation_rows": len(operations),
            "validated_restores": len(restore_rows),
            "manifest_entries": len(manifest_entries),
        },
        sort_keys=True,
    )
)
