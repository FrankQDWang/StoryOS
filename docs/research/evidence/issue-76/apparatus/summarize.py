#!/usr/bin/env python3
"""Create machine-readable summaries and claims from raw Issue #76 evidence."""

from __future__ import annotations

import csv
import json
from decimal import Decimal, ROUND_HALF_UP
from pathlib import Path

from distributions import (
    build_browser_distributions,
    build_operation_distributions,
    json_lines,
)


HERE = Path(__file__).resolve().parent
OUT = HERE.parent


def fixed(value: float, places: int) -> str:
    quantum = Decimal(1).scaleb(-places)
    return format(
        Decimal(str(value)).quantize(quantum, rounding=ROUND_HALF_UP),
        f".{places}f",
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
        encoding="utf-8", newline=""
    ) as handle:
        relation_rows = list(csv.DictReader(handle))

    browser_distributions = build_browser_distributions(browser_rows)
    operation_distributions = build_operation_distributions(operation_rows)

    families = (
        "revision_envelopes",
        "receipt_envelopes",
        "activity_event_envelopes",
        "application_wire_records",
    )
    loaded = [
        row
        for row in relation_rows
        if row["phase"] == "loaded"
        and any(row["relation"].startswith(family) for family in families)
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
        for profile in [entry["name"] for entry in workload["postgres"]["profiles"]]:
            components = {}
            for family in families:
                observed = [
                    value
                    for (candidate_profile, relation), value in slopes.items()
                    if candidate_profile == profile and relation.startswith(family)
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
        raise ValueError(f"expected one journal_growth row, got {len(journal_rows)}")
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

    def browser_stat(
        kind: str, group: str, metric: str
    ) -> dict[str, object]:
        matches = [
            row
            for row in browser_distributions
            if row["kind"] == kind
            and row["group"] == group
            and row["metric"] == metric
        ]
        if len(matches) != 1:
            raise ValueError(
                f"expected one browser distribution for {kind}/{group}/{metric}"
            )
        return matches[0]

    def operation_stat(
        profile: str, operation: str, metric: str
    ) -> dict[str, object]:
        matches = [
            row
            for row in operation_distributions
            if row["profile"] == profile
            and row["operation"] == operation
            and row["metric"] == metric
        ]
        if len(matches) != 1:
            raise ValueError(
                f"expected one operation distribution for "
                f"{profile}/{operation}/{metric}"
            )
        return matches[0]

    report_fragments: dict[str, str] = {}
    browser_environment_rows = [
        row for row in browser_rows if row["kind"] == "browser_environment"
    ]
    if len(browser_environment_rows) != 1:
        raise ValueError(
            "expected one browser_environment row, got "
            f"{len(browser_environment_rows)}"
        )
    browser_environment = browser_environment_rows[0]
    platform_version = str(environment["platform"]).split("-")[1]
    postgres_version = str(environment["postgres_version"]).split()[2]
    heap_limit_gib = (
        float(browser_environment["js_heap"]["js_heap_size_limit"]) / 2**30
    )
    novel_total_mb = (
        workload["browser"]["novel_chapters"]
        * workload["browser"]["novel_chapter_utf8_bytes"]
        / 1_000_000
    )
    record_counts_label = "/".join(
        f"{count // 1000}K" for count in workload["postgres"]["record_counts"]
    )
    replay_after_k = (
        workload["postgres"]["replay_events_after_checkpoint"] // 1000
    )
    compact_payload_kib = (
        workload["postgres"]["payload_bytes"]["compactable_event_payload"]
        // 1024
    )
    report_fragments.update(
        {
            "environment_runtime": (
                f"macOS {platform_version}, {environment['host_cpu']}, "
                f"{environment['host_logical_cpus']} logical CPUs, "
                f"{int(environment['host_memory_bytes']) // 2**30} GiB "
                f"host memory, Chrome `{browser_environment['browser_version']}`, "
                f"Python `{str(environment['python']).split()[0]}`, Docker server "
                f"`{environment['docker_server']}`, and PostgreSQL "
                f"`{postgres_version}`"
            ),
            "environment_postgres_image": (
                f"`{environment['postgres_image_id']}`"
            ),
            "environment_browser_heap": (
                f"a {heap_limit_gib:.1f} "
                f"GiB JavaScript heap limit"
            ),
            "workload_browser_input": (
                "into 10/50/200 KB `contenteditable`; "
                f"{workload['browser']['warmup_samples']} warm-ups then "
                f"{workload['browser']['measured_samples']} samples per chapter size"
            ),
            "workload_journal": (
                f"{workload['browser']['journal_intents']:,} sequential "
                f"{int(journal['patch_utf8_bytes_actual'])}-byte UTF-8 patches "
                f"with a full checkpoint every "
                f"{workload['browser']['journal_checkpoint_every']} intents"
            ),
            "workload_novel": (
                f"{workload['browser']['novel_chapters']} decimal-text chapters × "
                f"{workload['browser']['novel_chapter_utf8_bytes'] // 1000} KB = "
                f"{novel_total_mb:.1f} "
                f"MB; {workload['browser']['chapter_switch_samples']} switches and "
                f"{workload['browser']['cold_open_samples']} page reload/cold opens"
            ),
            "workload_postgres_growth": (
                f"{record_counts_label} "
                "records per Revision, Receipt, Activity Event, and Application "
                "Wire Record family"
            ),
            "workload_replay": (
                f"{workload['postgres']['replay_events_without_checkpoint'] // 1000}K "
                f"Event scan versus {replay_after_k}K "
                "Event tail after one checkpoint"
            ),
            "workload_compaction": (
                f"{workload['postgres']['record_counts'][-1] // 1000}K "
                f"{compact_payload_kib}-KiB "
                f"Event payloads; delete "
                f"{(1 - workload['postgres']['compaction_keep_fraction']) * 100:.0f}%"
            ),
        }
    )
    browser_specs = (
        (
            "browser_input_10k",
            "10 KB input → double `requestAnimationFrame`",
            "input_and_journal",
            "10000",
            "input_to_double_raf_ms",
            "Frame-boundary surrogate",
        ),
        (
            "browser_input_50k",
            "50 KB input → double `requestAnimationFrame`",
            "input_and_journal",
            "50000",
            "input_to_double_raf_ms",
            "Frame-boundary surrogate",
        ),
        (
            "browser_input_200k",
            "200 KB input → double `requestAnimationFrame`",
            "input_and_journal",
            "200000",
            "input_to_double_raf_ms",
            "Non-monotonic phase/cache evidence",
        ),
        (
            "browser_journal_10k",
            "10 KB strict IndexedDB `complete`",
            "input_and_journal",
            "10000",
            "strict_journal_ms",
            "Browser-visible strict-hint transaction completion",
        ),
        (
            "browser_journal_50k",
            "50 KB strict IndexedDB `complete`",
            "input_and_journal",
            "50000",
            "strict_journal_ms",
            "Same",
        ),
        (
            "browser_journal_200k",
            "200 KB strict IndexedDB `complete`",
            "input_and_journal",
            "200000",
            "strict_journal_ms",
            "Same",
        ),
        (
            "browser_synthetic_composition_paint",
            "Synthetic composition → double rAF",
            "synthetic_composition",
            "all",
            "input_to_double_raf_ms",
            "Not OS IME",
        ),
        (
            "browser_synthetic_composition_journal",
            "Synthetic composition journal `complete`",
            "synthetic_composition",
            "all",
            "strict_journal_ms",
            "Not OS IME",
        ),
        (
            "browser_offline_journal",
            "Offline journal `complete`",
            "offline_journal",
            "all",
            "strict_journal_ms",
            "Unreachable loopback, journal still local",
        ),
        (
            "browser_chapter_switch",
            "20 KB chapter switch → double rAF",
            "chapter_switch",
            "all",
            "load_to_double_raf_ms",
            "IndexedDB lookup plus replacement",
        ),
        (
            "browser_cold_open",
            "20 KB cold reload/open → double rAF",
            "cold_open",
            "all",
            "load_to_double_raf_ms",
            "Warm browser/profile, page reload",
        ),
    )
    for claim_id, label, kind, group, metric, interpretation in browser_specs:
        row = browser_stat(kind, group, metric)
        report_fragments[claim_id] = (
            f"| {label} | {row['n']} | {row['p50']:.1f} ms | "
            f"{row['p95']:.1f} ms | {row['p99']:.1f} ms | "
            f"{interpretation} |"
        )

    input_groups = ("10000", "50000", "200000")
    strict_journal_p95 = [
        float(browser_stat("input_and_journal", group, "strict_journal_ms")["p95"])
        for group in input_groups
    ]
    double_raf_p95 = [
        float(
            browser_stat(
                "input_and_journal", group, "input_to_double_raf_ms"
            )["p95"]
        )
        for group in input_groups
    ]
    switch_p95 = float(
        browser_stat("chapter_switch", "all", "load_to_double_raf_ms")["p95"]
    )
    cold_open_p95 = float(
        browser_stat("cold_open", "all", "load_to_double_raf_ms")["p95"]
    )
    report_fragments.update(
        {
            "recommendation_journal_evidence": (
                f"Strict IndexedDB p95 `{min(strict_journal_p95):.1f}-"
                f"{max(strict_journal_p95):.1f} ms`"
            ),
            "recommendation_paint_evidence": (
                f"Double-rAF p95 `{min(double_raf_p95):.1f}-"
                f"{max(double_raf_p95):.1f} ms`, switch p95 "
                f"`{switch_p95:.1f} ms`, cold reload p95 "
                f"`{cold_open_p95:.1f} ms`"
            ),
        }
    )

    network_specs = (
        ("network_loopback", "5 ms", "loopback", "loopback acknowledgement first"),
        (
            "network_ack_first_30",
            "30 ms",
            "ack_first_30",
            "acknowledgement first",
        ),
        ("network_event_first_30", "30 ms", "event_first_30", "Event first"),
        (
            "network_ack_first_100",
            "100 ms",
            "ack_first_100",
            "acknowledgement first",
        ),
        ("network_event_first_100", "100 ms", "event_first_100", "Event first"),
        (
            "network_ack_first_250",
            "250 ms",
            "ack_first_250",
            "acknowledgement first",
        ),
        ("network_event_first_250", "250 ms", "event_first_250", "Event first"),
    )
    for claim_id, slowest, group, order in network_specs:
        row = browser_stat("network_convergence", group, "convergence_ms")
        report_fragments[claim_id] = (
            f"| {slowest} | {order} | {row['n']} | "
            f"{row['p50']:.1f} ms | {row['p95']:.1f} ms | "
            f"{row['max']:.1f} ms |"
        )

    network_rows = [
        row for row in browser_rows if row["kind"] == "network_convergence"
    ]
    network_overheads = [
        float(row["convergence_ms"])
        - max(float(row["ack_delay_ms"]), float(row["event_delay_ms"]))
        for row in network_rows
    ]
    report_fragments["network_overhead_narrative"] = (
        "configured-delay overhead ranged "
        f"`{min(network_overheads):.1f}-{max(network_overheads):.1f} ms` "
        "in this run"
    )
    report_fragments["recommendation_network_evidence"] = (
        "Delayed loopback convergence followed the slower channel plus "
        f"`~{min(network_overheads):.1f}-{max(network_overheads):.1f} ms`"
    )

    report_fragments.update(
        {
            "journal_serialized_bytes": (
                f"- attributable serialized journal records: "
                f"`{int(journal['logical_serialized_bytes']):,} bytes`;"
            ),
            "journal_naive_bytes": (
                f"- hypothetical before-plus-after full-document copies: "
                f"`{critical_claims['journal_naive_before_after_full_copy_bytes']:,} "
                f"bytes`;"
            ),
            "journal_amplification": (
                f"`{critical_claims['journal_naive_amplification']:.1f}×` relative "
                f"to that deliberately naive comparator."
            ),
            "journal_storage_estimate": (
                f"fell from "
                f"`{int(journal['storage_estimate_before']['usage']):,}` to "
                f"`{int(journal['storage_estimate_after']['usage']):,}` bytes"
            ),
        }
    )

    relation_specs = (
        ("postgres_relation_revision", "Revision, 1,024 B", "revision_envelopes"),
        ("postgres_relation_receipt", "Receipt, 640 B", "receipt_envelopes"),
        (
            "postgres_relation_event",
            "Activity Event, 768 B",
            "activity_event_envelopes",
        ),
        (
            "postgres_relation_wire",
            "Application Wire Record, 1,024 B",
            "application_wire_records",
        ),
    )
    for claim_id, label, family in relation_specs:
        matches = [
            row
            for row in relation_rows
            if row["profile"] == "local_4vcpu_4gib"
            and row["sample"] == "0"
            and row["phase"] == "loaded"
            and row["record_count_per_family"] == "50000"
            and row["relation"] == f"{family}_compressible"
        ]
        if len(matches) != 1:
            raise ValueError(f"expected one 50K relation row for {family}")
        row = matches[0]
        report_fragments[claim_id] = (
            f"| {label} | {int(row['heap_bytes']):,} B | "
            f"{int(row['index_bytes']):,} B | {int(row['total_bytes']):,} B | "
            f"{int(row['total_bytes']) / 50_000:,.3f} B at 50K |"
        )

    family_slope = {}
    for family in families:
        observed = [
            value
            for (_, relation), value in slopes.items()
            if relation.startswith(family)
        ]
        family_slope[family] = max(observed)
    report_fragments["postgres_upper_slopes"] = (
        f"`{family_slope['revision_envelopes']:,.3f}`, "
        f"`{family_slope['receipt_envelopes']:,.3f}`, "
        f"`{family_slope['activity_event_envelopes']:,.3f}`, and "
        f"`{family_slope['application_wire_records']:,.3f}` bytes/record"
    )
    shape_low = min(shape_probe_slopes.values())
    shape_high = max(shape_probe_slopes.values())
    report_fragments["postgres_shape_range"] = (
        f"ranged from `{shape_low:,.3f}` for highly compressible values to "
        f"`{shape_high:,.3f}` for low-compressibility values"
    )
    report_fragments["postgres_shape_ratio"] = f"`{shape_high / shape_low:.2f}×` span"
    report_fragments["executive_shape_sensitivity"] = (
        f"a 4 KiB payload sensitivity probe ranged from `{shape_low:,.3f}` to "
        f"`{shape_high:,.3f}` total-relation bytes per record"
    )
    report_fragments["recommendation_storage_slope"] = (
        f"Four small families model `{sum(family_slope.values()):,.3f} "
        f"B/command`; 4-KiB sensitivity implies up to "
        f"`{shape_high:,.3f} B/record`"
    )

    for model in models:
        if model["resource_profile"] != "local_4vcpu_4gib":
            continue
        components = model["component_byte_ranges"]
        commands = int(model["author_commands_per_year"])
        report_fragments[f"storage_model_{commands}"] = (
            f"| {commands:,} | "
            f"{fixed(components['revision_envelopes']['high_bytes'] / 2**20, 2)} "
            f"MiB | "
            f"{fixed(components['receipt_envelopes']['high_bytes'] / 2**20, 2)} "
            f"MiB | "
            f"{fixed(components['activity_event_envelopes']['high_bytes'] / 2**20, 2)} "
            f"MiB | "
            f"{fixed(components['application_wire_records']['high_bytes'] / 2**20, 2)} "
            f"MiB | {model['modeled_total_high_bytes'] / 2**20:.2f} MiB |"
        )
    local_models = {
        int(model["author_commands_per_year"]): model
        for model in models
        if model["resource_profile"] == "local_4vcpu_4gib"
    }
    report_fragments["executive_storage_models"] = (
        f"about `{local_models[20_000]['modeled_total_high_bytes'] / 2**20:.1f} "
        f"MiB` for 20,000 annual commands and "
        f"`{local_models[60_000]['modeled_total_high_bytes'] / 2**20:.1f} MiB` "
        "for 60,000 annual commands"
    )
    report_fragments["recommendation_storage_models"] = (
        f"20K/60K commands model "
        f"`{local_models[20_000]['modeled_total_high_bytes'] / 2**20:.1f}/"
        f"{local_models[60_000]['modeled_total_high_bytes'] / 2**20:.1f} "
        "MiB/year`"
    )

    wal_rows = [
        row for row in operation_rows if row["operation"] == "loaded_wal_growth"
    ]
    for count, probe in (
        (1_000, "no"),
        (10_000, "10K rows × two shapes"),
        (50_000, "no"),
    ):
        values = [
            int(row["wal_delta_bytes"])
            for row in wal_rows
            if row["record_count_per_family"] == count
        ]
        rendered = (
            f"{min(values):,} B"
            if min(values) == max(values)
            else f"{min(values):,}-{max(values):,} B"
        )
        report_fragments[f"wal_{count}"] = (
            f"| {count:,} | {probe} | {rendered} |"
        )

    replay_profiles = (
        ("local_4vcpu_4gib", "4 vCPU / 4 GiB"),
        (
            "controlled_cloud_surrogate_2vcpu_2gib",
            "2 vCPU / 2 GiB surrogate",
        ),
    )
    for profile, label in replay_profiles:
        full = operation_stat(profile, "checkpoint_replay_scan", "full_replay_ms")
        bounded = operation_stat(
            profile, "checkpoint_replay_scan", "bounded_replay_ms"
        )
        report_fragments[f"replay_{profile}"] = (
            f"| {label} | {full['n']} | {full['p50']:.3f}/{full['max']:.3f} ms | "
            f"{bounded['p50']:.3f}/{bounded['max']:.3f} ms |"
        )

    relation_phase = {
        (row["phase"], row["relation"]): int(row["total_bytes"])
        for row in relation_rows
        if row["profile"] == "local_4vcpu_4gib" and row["sample"] == "0"
    }
    event_pre = relation_phase[("pre_compaction", "event_payloads")]
    event_vacuum = relation_phase[("post_delete_vacuum", "event_payloads")]
    event_full = relation_phase[("post_vacuum_full", "event_payloads")]
    floor_pre = relation_phase[("pre_compaction", "compaction_floors")]
    floor_full = relation_phase[("post_vacuum_full", "compaction_floors")]
    net_pre = event_pre + floor_pre
    net_full = event_full + floor_full
    report_fragments.update(
        {
            "compaction_event_sizes": (
                f"changed total relation bytes from `{event_pre:,}` to "
                f"`{event_vacuum:,}`"
            ),
            "compaction_event_rewrite": (
                f"`VACUUM FULL` rewrote it to `{event_full:,}` bytes, a "
                f"`{(1 - event_full / event_pre) * 100:.2f}%` reduction"
            ),
            "compaction_floor_size": (
                f"records occupied another `{floor_full:,}` bytes"
            ),
            "compaction_net_size": (
                f"Event plus floor fell from `{net_pre:,}` to `{net_full:,}` "
                f"bytes, an `{(1 - net_full / net_pre) * 100:.2f}%` net reduction"
            ),
            "compaction_delete_range": (
                f"`{critical_claims['delete_and_floor_ms']['min']:.1f}-"
                f"{critical_claims['delete_and_floor_ms']['max']:.1f} ms`"
            ),
            "compaction_vacuum_range": (
                f"`{critical_claims['vacuum_full_ms']['min']:.1f}-"
                f"{critical_claims['vacuum_full_ms']['max']:.1f} ms`"
            ),
            "compaction_load_wal": (
                f"`{min(row['load_wal_delta_bytes'] for row in compaction_rows):,}` "
                f"WAL bytes"
            ),
            "compaction_wal_range": (
                f"`{critical_claims['compaction_wal_delta_bytes']['min']:,}-"
                f"{critical_claims['compaction_wal_delta_bytes']['max']:,}` "
                f"WAL bytes"
            ),
            "recommendation_compaction_evidence": (
                f"{(1 - workload['postgres']['compaction_keep_fraction']) * 100:.0f}% "
                "delete + ordinary vacuum did not shrink files; rewrite reduced "
                f"sampled Event relation "
                f"`{(1 - event_full / event_pre) * 100:.2f}%`"
            ),
        }
    )

    dump_times = [float(row["dump_ms"]) for row in restore_rows]
    restore_times = [float(row["restore_ms"]) for row in restore_rows]
    source_sizes = {int(row["source_database_bytes"]) for row in restore_rows}
    if len(source_sizes) != 1:
        raise ValueError(f"source database sizes differ: {source_sizes}")
    report_fragments.update(
        {
            "restore_source_size": (
                f"source database was `{source_sizes.pop():,}` bytes"
            ),
            "restore_dump_range": (
                f"logical dumps were "
                f"`{critical_claims['dump_bytes']['min']:,}-"
                f"{critical_claims['dump_bytes']['max']:,}` bytes"
            ),
            "restore_dump_time": (
                f"Dump duration was `{min(dump_times) / 1000:.3f}-"
                f"{max(dump_times) / 1000:.3f} s`"
            ),
            "restore_time": (
                f"restore was `{min(restore_times) / 1000:.3f}-"
                f"{max(restore_times) / 1000:.3f} s`"
            ),
            "restore_identity": (
                f"exactly "
                f"{int(restore_rows[0]['source_count_and_sequence_sum'].split()[0]):,} "
                "Event rows and sequence sum "
                f"`"
                f"{int(restore_rows[0]['source_count_and_sequence_sum'].split()[1]):,}"
                "`"
            ),
            "recommendation_restore_evidence": (
                f"{len(restore_rows)} logical restores matched count and sequence sum"
            ),
            "resource_samples": (
                f"All {len(restore_rows)} database samples completed inside the "
                f"declared {workload['postgres']['profiles'][0]['cpus']}-vCPU/"
                f"{workload['postgres']['profiles'][0]['memory'].replace('g', '-GiB')} "
                f"and {workload['postgres']['profiles'][1]['cpus']}-vCPU/"
                f"{workload['postgres']['profiles'][1]['memory'].replace('g', '-GiB')} "
                "container caps"
            ),
        }
    )
    measured_browser_group_sizes = [
        int(row["n"])
        for row in browser_distributions
        if row["kind"]
        in {
            "input_and_journal",
            "synthetic_composition",
            "offline_journal",
            "chapter_switch",
            "cold_open",
            "network_convergence",
        }
    ]
    report_fragments["recommendation_sample_evidence"] = (
        f"Browser distributions have n={min(measured_browser_group_sizes)}-"
        f"{max(measured_browser_group_sizes)}; database distributions "
        f"n={workload['postgres']['measured_runs']}"
    )
    report_fragments["verification_coverage"] = (
        f"`summary.json` now carries {len(report_fragments) + 1} "
        "raw-derived report fragments"
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
