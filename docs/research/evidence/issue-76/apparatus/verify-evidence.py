#!/usr/bin/env python3
"""Validate a fresh Issue #76 run or audit the frozen evidence bundle."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import subprocess
from pathlib import Path

from distributions import json_lines
from evidence_files import included_evidence_paths
from summarize import build_summary


HERE = Path(__file__).resolve().parent
OUT = HERE.parent
REPORT = OUT.parent.parent / (
    "representative-writing-path-performance-and-storage-growth-envelope.md"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode",
        choices=("fresh", "frozen"),
        default="frozen",
        help=(
            "fresh validates a newly measured raw bundle; frozen additionally "
            "audits the checked-in report and comparator-correction provenance"
        ),
    )
    return parser.parse_args()


def verify_report(
    rebuilt_summary: dict[str, object],
    errors: list[str],
) -> None:
    report = REPORT.read_text(encoding="utf-8")
    normalized_report = re.sub(r"\s+", " ", report)
    for claim, fragment in rebuilt_summary["report_fragments"].items():
        if re.sub(r"\s+", " ", fragment) not in normalized_report:
            errors.append(f"report {claim} missing derived fragment: {fragment}")


def verify_correction_provenance(
    journal: dict[str, object],
    expected_naive_bytes: int,
    errors: list[str],
) -> dict[str, object]:
    provenance = json.loads(
        (OUT / "browser-evidence-correction.json").read_text(encoding="utf-8")
    )
    source_bytes = subprocess.run(
        [
            "git",
            "show",
            (
                f"{provenance['source_main_commit']}:"
                "docs/research/evidence/issue-76/browser-measurements.jsonl"
            ),
        ],
        check=True,
        capture_output=True,
    ).stdout
    current_bytes = (OUT / "browser-measurements.jsonl").read_bytes()
    source_lines = source_bytes.splitlines()
    current_lines = current_bytes.splitlines()
    target_index = int(provenance["target_zero_based_line"])
    source_target = json.loads(source_lines[target_index])
    current_target = json.loads(current_lines[target_index])
    source_expected_twice_post_edit = (
        int(source_target["patch_utf8_bytes_actual"])
        * int(source_target["intent_count"])
        * (int(source_target["intent_count"]) + 1)
    )
    if (
        hashlib.sha256(source_bytes).hexdigest()
        != provenance["source_raw_sha256"]
    ):
        errors.append("browser correction source SHA-256 does not match source main")
    if (
        hashlib.sha256(current_bytes).hexdigest()
        != provenance["corrected_raw_sha256"]
    ):
        errors.append("browser correction output SHA-256 does not match current raw")
    if len(source_lines) != len(current_lines) or any(
        source_line != current_lines[index]
        for index, source_line in enumerate(source_lines)
        if index != target_index
    ):
        errors.append("browser correction changed rows outside journal_growth")
    unaffected_bytes = b"\n".join(
        line
        for index, line in enumerate(current_lines)
        if index != target_index
    ) + b"\n"
    if (
        hashlib.sha256(unaffected_bytes).hexdigest()
        != provenance["unaffected_rows_sha256"]
    ):
        errors.append(
            "browser correction unaffected-row aggregate SHA-256 mismatch"
        )
    if provenance["unchanged_row_count"] != len(current_lines) - 1:
        errors.append("browser correction unchanged-row count mismatch")
    if (
        source_target["naive_before_after_full_copy_bytes"]
        != source_expected_twice_post_edit
        or provenance["previous_twice_post_edit_bytes"]
        != source_expected_twice_post_edit
    ):
        errors.append(
            "browser correction source is not the known twice-post-edit value"
        )
    if (
        current_target["naive_before_after_full_copy_bytes"]
        != expected_naive_bytes
        or provenance["corrected_before_plus_after_bytes"]
        != expected_naive_bytes
    ):
        errors.append(
            "browser correction target is not the before-plus-after value"
        )
    if (
        provenance["intent_count"] != journal["intent_count"]
        or provenance["patch_utf8_bytes_actual"]
        != journal["patch_utf8_bytes_actual"]
    ):
        errors.append("browser correction formula inputs do not match the raw row")
    return provenance


def verify_manifest(errors: list[str]) -> dict[str, str]:
    manifest_entries = {}
    manifest_lines = (OUT / "MANIFEST.sha256").read_text(
        encoding="utf-8"
    ).splitlines()
    for line in manifest_lines:
        digest, relative = line.split("  ", 1)
        if relative in manifest_entries:
            errors.append(f"manifest contains duplicate entry: {relative}")
        manifest_entries[relative] = digest
    if any(".reference" in relative for relative in manifest_entries):
        errors.append("manifest must not include .reference paths")

    expected_entries = {
        str(path.relative_to(OUT)) for path in included_evidence_paths(OUT)
    }
    if set(manifest_entries) != expected_entries:
        errors.append(
            "manifest file set mismatch: "
            f"missing={sorted(expected_entries - set(manifest_entries))} "
            f"extra={sorted(set(manifest_entries) - expected_entries)}"
        )
    for relative, digest in manifest_entries.items():
        actual_digest = hashlib.sha256((OUT / relative).read_bytes()).hexdigest()
        if actual_digest != digest:
            errors.append(
                f"manifest digest mismatch for {relative}: "
                f"manifest={digest} actual={actual_digest}"
            )
    return manifest_entries


def main() -> int:
    args = parse_args()
    workload = json.loads((HERE / "workload.json").read_text(encoding="utf-8"))
    browser = json_lines(OUT / "browser-measurements.jsonl")
    operations = json_lines(OUT / "postgres-operations.jsonl")
    summary = json.loads((OUT / "summary.json").read_text(encoding="utf-8"))
    environment = json.loads(
        (OUT / "environment.json").read_text(encoding="utf-8")
    )
    with (OUT / "postgres-relation-growth.csv").open(
        encoding="utf-8",
        newline="",
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
    assert all(
        row["schema"] == "storyos.issue76.browser.v1" for row in browser
    )
    assert all(
        row["schema"] == "storyos.issue76.postgres_operation.v1"
        for row in operations
    )
    assert {row["sample"] for row in operations} == {0, 1, 2}
    assert {
        row["profile"] for row in operations
    } == {
        "local_4vcpu_4gib",
        "controlled_cloud_surrogate_2vcpu_2gib",
    }

    restore_rows = [
        row
        for row in operations
        if row["operation"] == "logical_backup_restore"
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

    errors: list[str] = []
    rebuilt_summary = build_summary(OUT)
    if summary != rebuilt_summary:
        errors.append(
            "summary.json is inconsistent with a full rebuild from raw evidence"
        )

    journal_rows = [
        row for row in browser if row["kind"] == "journal_growth"
    ]
    assert len(journal_rows) == 1
    journal = journal_rows[0]
    expected_naive_bytes = (
        int(journal["patch_utf8_bytes_actual"])
        * int(journal["intent_count"]) ** 2
    )
    if journal["naive_before_after_full_copy_bytes"] != expected_naive_bytes:
        errors.append(
            "journal comparator mismatch: "
            f"raw={journal['naive_before_after_full_copy_bytes']} "
            f"expected_before_plus_after={expected_naive_bytes}"
        )

    compaction_rows = [
        row for row in operations if row["operation"] == "compaction"
    ]
    assert len(compaction_rows) == 6
    critical_claims = {
        "journal_naive_before_after_full_copy_bytes": expected_naive_bytes,
        "journal_naive_amplification": expected_naive_bytes
        / int(journal["logical_serialized_bytes"]),
        "delete_and_floor_ms": {
            "min": min(
                row["delete_and_floor_ms"] for row in compaction_rows
            ),
            "max": max(
                row["delete_and_floor_ms"] for row in compaction_rows
            ),
        },
        "vacuum_full_ms": {
            "min": min(row["vacuum_full_ms"] for row in compaction_rows),
            "max": max(row["vacuum_full_ms"] for row in compaction_rows),
        },
        "compaction_wal_delta_bytes": {
            "min": min(
                row["compaction_wal_delta_bytes"] for row in compaction_rows
            ),
            "max": max(
                row["compaction_wal_delta_bytes"] for row in compaction_rows
            ),
        },
        "dump_bytes": {
            "min": min(row["dump_bytes"] for row in restore_rows),
            "max": max(row["dump_bytes"] for row in restore_rows),
        },
    }
    if summary.get("critical_claims") != critical_claims:
        errors.append(
            "summary critical_claims missing or inconsistent with raw evidence"
        )

    provenance = None
    if args.mode == "frozen":
        verify_report(rebuilt_summary, errors)
        provenance = verify_correction_provenance(
            journal,
            expected_naive_bytes,
            errors,
        )
    manifest_entries = verify_manifest(errors)
    if errors:
        raise AssertionError("\n".join(errors))

    result = {
        "browser_rows": len(browser),
        "manifest_entries": len(manifest_entries),
        "postgres_operation_rows": len(operations),
        "postgres_relation_rows": len(relations),
        "validated_restores": len(restore_rows),
    }
    if args.mode == "frozen":
        result.update(
            {
                "report_fragments": len(rebuilt_summary["report_fragments"]),
                "unchanged_browser_rows": provenance["unchanged_row_count"],
            }
        )
    else:
        result["mode"] = "fresh"
        result["scope"] = "measurement_bundle_only"
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
