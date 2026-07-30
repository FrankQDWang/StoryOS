#!/usr/bin/env python3
"""Create machine-readable summaries and claims from raw Issue #76 evidence."""

from __future__ import annotations

import csv
import json
from pathlib import Path

from distributions import (
    build_browser_distributions,
    build_operation_distributions,
    json_lines,
)
from report_fragments import build_report_fragments


HERE = Path(__file__).resolve().parent
OUT = HERE.parent
FAMILIES = (
    "revision_envelopes",
    "receipt_envelopes",
    "activity_event_envelopes",
    "application_wire_records",
)


def build_summary(out: Path = OUT) -> dict[str, object]:
    workload = json.loads(
        (out / "apparatus" / "workload.json").read_text(encoding="utf-8")
    )
    environment = json.loads(
        (out / "environment.json").read_text(encoding="utf-8")
    )
    browser_rows = json_lines(out / "browser-measurements.jsonl")
    operation_rows = json_lines(out / "postgres-operations.jsonl")
    with (out / "postgres-relation-growth.csv").open(
        encoding="utf-8",
        newline="",
    ) as handle:
        relation_rows = list(csv.DictReader(handle))

    browser_distributions = build_browser_distributions(browser_rows)
    operation_distributions = build_operation_distributions(operation_rows)
    loaded = [
        row
        for row in relation_rows
        if row["phase"] == "loaded"
        and any(row["relation"].startswith(family) for family in FAMILIES)
    ]
    slopes: dict[tuple[str, str], float] = {}
    for row in loaded:
        count = int(row["record_count_per_family"])
        key = (row["profile"], row["relation"])
        slopes[key] = max(
            slopes.get(key, 0.0),
            int(row["total_bytes"]) / count,
        )

    shape_probe_rows = [
        row
        for row in relation_rows
        if row["phase"] == "loaded"
        and row["record_count_per_family"] == "10000"
        and row["relation"].startswith("payload_shape_probe_")
    ]
    shape_probe_slopes: dict[tuple[str, str], float] = {}
    for row in shape_probe_rows:
        key = (row["profile"], row["relation"])
        shape_probe_slopes[key] = max(
            shape_probe_slopes.get(key, 0.0),
            int(row["total_bytes"]) / int(row["record_count_per_family"]),
        )

    models = []
    for project in workload["modeled_project_years"]:
        profiles = [entry["name"] for entry in workload["postgres"]["profiles"]]
        for profile in profiles:
            components = {}
            for family in FAMILIES:
                observed = [
                    value
                    for (candidate_profile, relation), value in slopes.items()
                    if candidate_profile == profile
                    and relation.startswith(family)
                ]
                components[family] = {
                    "low_bytes": min(observed)
                    * project["author_commands_per_year"],
                    "high_bytes": max(observed)
                    * project["author_commands_per_year"],
                }
            models.append(
                {
                    "schema": "storyos.issue76.storage_model.v1",
                    "project_profile": project["name"],
                    "resource_profile": profile,
                    "author_commands_per_year": project[
                        "author_commands_per_year"
                    ],
                    "component_byte_ranges": components,
                    "modeled_total_low_bytes": sum(
                        item["low_bytes"] for item in components.values()
                    ),
                    "modeled_total_high_bytes": sum(
                        item["high_bytes"] for item in components.values()
                    ),
                    "method": (
                        "observed compressible-to-low-compressibility "
                        "pg_total_relation_size per row at 1k/10k/50k, "
                        "multiplied linearly"
                    ),
                    "uncertainty": (
                        "synthetic schema and payload distributions; excludes "
                        "future tables, WAL, backups, bloat, archives, and "
                        "browser journal"
                    ),
                }
            )

    journal_rows = [
        row for row in browser_rows if row["kind"] == "journal_growth"
    ]
    if len(journal_rows) != 1:
        raise ValueError(
            f"expected one journal_growth row, got {len(journal_rows)}"
        )
    journal = journal_rows[0]
    compaction_rows = [
        row for row in operation_rows if row["operation"] == "compaction"
    ]
    restore_rows = [
        row
        for row in operation_rows
        if row["operation"] == "logical_backup_restore"
    ]
    critical_claims = {
        "journal_naive_before_after_full_copy_bytes": int(
            journal["naive_before_after_full_copy_bytes"]
        ),
        "journal_naive_amplification": (
            int(journal["naive_before_after_full_copy_bytes"])
            / int(journal["logical_serialized_bytes"])
        ),
        "delete_and_floor_ms": {
            "min": min(row["delete_and_floor_ms"] for row in compaction_rows),
            "max": max(row["delete_and_floor_ms"] for row in compaction_rows),
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
    report_fragments = build_report_fragments(
        workload=workload,
        environment=environment,
        browser_rows=browser_rows,
        operation_rows=operation_rows,
        relation_rows=relation_rows,
        browser_distributions=browser_distributions,
        operation_distributions=operation_distributions,
        families=FAMILIES,
        slopes=slopes,
        shape_probe_slopes=shape_probe_slopes,
        models=models,
        journal=journal,
        compaction_rows=compaction_rows,
        restore_rows=restore_rows,
        critical_claims=critical_claims,
    )
    return {
        "schema": "storyos.issue76.summary.v1",
        "browser_distributions": browser_distributions,
        "postgres_operation_distributions": operation_distributions,
        "postgres_bytes_per_record_upper_observed": [
            {
                "profile": profile,
                "relation": relation,
                "bytes_per_record": value,
            }
            for (profile, relation), value in sorted(slopes.items())
        ],
        "postgres_4k_payload_shape_probe_bytes_per_record": [
            {
                "profile": profile,
                "relation": relation,
                "bytes_per_record": value,
            }
            for (profile, relation), value in sorted(shape_probe_slopes.items())
        ],
        "storage_models": models,
        "critical_claims": critical_claims,
        "report_fragments": report_fragments,
    }


def main() -> int:
    summary = build_summary()
    (OUT / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "browser_summaries": len(summary["browser_distributions"]),
                "storage_models": len(summary["storage_models"]),
                "critical_claims": len(summary["critical_claims"]),
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
