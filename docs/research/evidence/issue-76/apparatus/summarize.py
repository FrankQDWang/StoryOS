#!/usr/bin/env python3
"""Create machine-readable summaries and modeled ranges from raw Issue #76 evidence."""

from __future__ import annotations

import csv
import json
import math
import statistics
from collections import defaultdict
from pathlib import Path


HERE = Path(__file__).resolve().parent
OUT = HERE.parent
WORKLOAD = json.loads((HERE / "workload.json").read_text(encoding="utf-8"))


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


browser_rows = [
    json.loads(line)
    for line in (OUT / "browser-measurements.jsonl").read_text(encoding="utf-8").splitlines()
]
groups: dict[tuple[str, str], list[dict[str, object]]] = defaultdict(list)
for row in browser_rows:
    key = (
        row["kind"],
        str(row.get("profile", row.get("chapter_profile_utf8_bytes", "all"))),
    )
    groups[key].append(row)

summaries: list[dict[str, object]] = []
for (kind, group), rows in sorted(groups.items()):
    numeric_keys = sorted(
        {
            key
            for row in rows
            for key, value in row.items()
            if isinstance(value, (int, float)) and key not in {"sample", "sequence"}
        }
    )
    for metric in numeric_keys:
        values = [float(row[metric]) for row in rows if isinstance(row.get(metric), (int, float))]
        if not values:
            continue
        summaries.append(
            {
                "source": "browser",
                "kind": kind,
                "group": group,
                "metric": metric,
                "n": len(values),
                "min": min(values),
                "p50": percentile(values, 0.50),
                "p95": percentile(values, 0.95),
                "p99": percentile(values, 0.99),
                "max": max(values),
                "mean": statistics.fmean(values),
            }
        )

with (OUT / "postgres-relation-growth.csv").open(encoding="utf-8", newline="") as handle:
    relation_rows = list(csv.DictReader(handle))

operation_rows = [
    json.loads(line)
    for line in (OUT / "postgres-operations.jsonl")
    .read_text(encoding="utf-8")
    .splitlines()
]
operation_groups: dict[tuple[str, str, str], list[dict[str, object]]] = defaultdict(
    list
)
for row in operation_rows:
    qualifiers = {
        key: row[key]
        for key in (
            "mode",
            "step",
            "relation",
            "record_count",
            "record_count_per_family",
            "payload_shape",
        )
        if key in row
    }
    group = json.dumps(qualifiers, sort_keys=True, separators=(",", ":"))
    operation_groups[(row["profile"], row["operation"], group)].append(row)

operation_summaries: list[dict[str, object]] = []
for (profile, operation, group), rows in sorted(operation_groups.items()):
    numeric_keys = sorted(
        {
            key
            for row in rows
            for key, value in row.items()
            if isinstance(value, (int, float))
            and key
            not in {
                "sample",
                "record_count",
                "record_count_per_family",
                "payload_bytes",
            }
        }
    )
    for metric in numeric_keys:
        values = [
            float(row[metric])
            for row in rows
            if isinstance(row.get(metric), (int, float))
        ]
        operation_summaries.append(
            {
                "source": "postgres",
                "profile": profile,
                "operation": operation,
                "group": group,
                "metric": metric,
                "n": len(values),
                "min": min(values),
                "p50": percentile(values, 0.50),
                "p95": percentile(values, 0.95),
                "p99": percentile(values, 0.99),
                "max": max(values),
                "mean": statistics.fmean(values),
            }
        )

loaded = [
    row
    for row in relation_rows
    if row["phase"] == "loaded"
    and any(
        row["relation"].startswith(family)
        for family in (
            "revision_envelopes",
            "receipt_envelopes",
            "activity_event_envelopes",
            "application_wire_records",
        )
    )
]
slopes: dict[tuple[str, str], float] = {}
for row in loaded:
    count = int(row["record_count_per_family"])
    slopes[(row["profile"], row["relation"])] = max(
        slopes.get((row["profile"], row["relation"]), 0.0),
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
    shape_probe_slopes[(row["profile"], row["relation"])] = max(
        shape_probe_slopes.get((row["profile"], row["relation"]), 0.0),
        int(row["total_bytes"]) / int(row["record_count_per_family"]),
    )

models = []
for project in WORKLOAD["modeled_project_years"]:
    for profile in [entry["name"] for entry in WORKLOAD["postgres"]["profiles"]]:
        components = {}
        for family in (
            "revision_envelopes",
            "receipt_envelopes",
            "activity_event_envelopes",
            "application_wire_records",
        ):
            observed = [
                value
                for (candidate_profile, relation), value in slopes.items()
                if candidate_profile == profile and relation.startswith(family)
            ]
            components[family] = {
                "low_bytes": min(observed) * project["author_commands_per_year"],
                "high_bytes": max(observed) * project["author_commands_per_year"],
            }
        models.append(
            {
                "schema": "storyos.issue76.storage_model.v1",
                "project_profile": project["name"],
                "resource_profile": profile,
                "author_commands_per_year": project["author_commands_per_year"],
                "component_byte_ranges": components,
                "modeled_total_low_bytes": sum(item["low_bytes"] for item in components.values()),
                "modeled_total_high_bytes": sum(item["high_bytes"] for item in components.values()),
                "method": "observed compressible-to-low-compressibility pg_total_relation_size per row at 1k/10k/50k, multiplied linearly",
                "uncertainty": "synthetic schema and payload distributions; excludes future tables, WAL, backups, bloat, archives, and browser journal",
            }
        )

(OUT / "summary.json").write_text(
    json.dumps(
        {
            "schema": "storyos.issue76.summary.v1",
            "browser_distributions": summaries,
            "postgres_operation_distributions": operation_summaries,
            "postgres_bytes_per_record_upper_observed": [
                {"profile": profile, "relation": relation, "bytes_per_record": value}
                for (profile, relation), value in sorted(slopes.items())
            ],
            "postgres_4k_payload_shape_probe_bytes_per_record": [
                {"profile": profile, "relation": relation, "bytes_per_record": value}
                for (profile, relation), value in sorted(shape_probe_slopes.items())
            ],
            "storage_models": models,
        },
        indent=2,
        sort_keys=True,
    )
    + "\n",
    encoding="utf-8",
)
print(json.dumps({"browser_summaries": len(summaries), "storage_models": len(models)}))
