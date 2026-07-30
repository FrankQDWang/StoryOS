"""Distribution helpers for the Issue #76 evidence derivation."""

from __future__ import annotations

import json
import math
import statistics
from collections import defaultdict
from pathlib import Path


def json_lines(path: Path) -> list[dict[str, object]]:
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line
    ]


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(
        0,
        min(len(ordered) - 1, math.ceil(fraction * len(ordered)) - 1),
    )
    return ordered[index]


def distribution(values: list[float]) -> dict[str, float | int]:
    return {
        "n": len(values),
        "min": min(values),
        "p50": percentile(values, 0.50),
        "p95": percentile(values, 0.95),
        "p99": percentile(values, 0.99),
        "max": max(values),
        "mean": statistics.fmean(values),
    }


def build_browser_distributions(
    browser_rows: list[dict[str, object]],
) -> list[dict[str, object]]:
    groups: dict[tuple[str, str], list[dict[str, object]]] = defaultdict(list)
    for row in browser_rows:
        key = (
            str(row["kind"]),
            str(
                row.get(
                    "profile",
                    row.get("chapter_profile_utf8_bytes", "all"),
                )
            ),
        )
        groups[key].append(row)

    summaries: list[dict[str, object]] = []
    for (kind, group), rows in sorted(groups.items()):
        numeric_keys = sorted(
            {
                key
                for row in rows
                for key, value in row.items()
                if isinstance(value, (int, float))
                and key not in {"sample", "sequence"}
            }
        )
        for metric in numeric_keys:
            values = [
                float(row[metric])
                for row in rows
                if isinstance(row.get(metric), (int, float))
            ]
            if values:
                summaries.append(
                    {
                        "source": "browser",
                        "kind": kind,
                        "group": group,
                        "metric": metric,
                        **distribution(values),
                    }
                )
    return summaries


def build_operation_distributions(
    operation_rows: list[dict[str, object]],
) -> list[dict[str, object]]:
    groups: dict[tuple[str, str, str], list[dict[str, object]]] = defaultdict(
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
        groups[(str(row["profile"]), str(row["operation"]), group)].append(row)

    summaries: list[dict[str, object]] = []
    for (profile, operation, group), rows in sorted(groups.items()):
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
            summaries.append(
                {
                    "source": "postgres",
                    "profile": profile,
                    "operation": operation,
                    "group": group,
                    "metric": metric,
                    **distribution(values),
                }
            )
    return summaries
