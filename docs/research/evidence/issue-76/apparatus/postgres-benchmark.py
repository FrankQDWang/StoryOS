#!/usr/bin/env python3
"""Disposable PostgreSQL storage-growth instrument for StoryOS Issue #76."""

from __future__ import annotations

import csv
import hashlib
import json
import platform
import resource
import subprocess
import sys
import time
from pathlib import Path


HERE = Path(__file__).resolve().parent
OUT = HERE.parent
WORKLOAD = json.loads((HERE / "workload.json").read_text(encoding="utf-8"))
IMAGE = WORKLOAD["postgres"]["image"]
CONTAINER = "storyos-issue76-postgres"
PASSWORD = "issue76-disposable-only"


def run(args: list[str], *, stdin: str | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        input=stdin,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(args)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def docker(*args: str, stdin: str | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(["docker", *args], stdin=stdin, check=check)


def cleanup_containers() -> None:
    ids = docker(
        "ps", "-aq", "--filter", "label=storyos.issue76=true", check=False
    ).stdout.split()
    if ids:
        docker("rm", "-f", *ids, check=False)


def psql(sql: str, *, database: str = "storyos_issue76", tuples: bool = False) -> str:
    args = ["exec", "-i", CONTAINER, "psql", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", database]
    if tuples:
        args.extend(["-At", "-F", "\t"])
    result = docker(*args, stdin=sql)
    return result.stdout


def wait_ready() -> None:
    for _ in range(60):
        log_result = docker("logs", CONTAINER, check=False)
        logs = log_result.stdout + log_result.stderr
        final_server_started = logs.count("database system is ready to accept connections") >= 2
        ready = (
            docker(
                "exec",
                CONTAINER,
                "pg_isready",
                "-U",
                "postgres",
                check=False,
            ).returncode
            == 0
        )
        if final_server_started and ready:
            return
        time.sleep(0.5)
    raise RuntimeError("PostgreSQL container did not become ready")


def relation_metrics(
    profile: str, sample: int, phase: str, count: int
) -> list[dict[str, object]]:
    output = psql(
        """
SELECT relname,
       pg_relation_size(oid),
       pg_indexes_size(oid),
       pg_total_relation_size(oid)
FROM pg_class
WHERE relnamespace = 'issue76'::regnamespace
  AND relkind = 'r'
ORDER BY relname;
""",
        tuples=True,
    )
    rows = []
    for line in output.splitlines():
        name, heap, indexes, total = line.split("\t")
        rows.append(
            {
                "schema": "storyos.issue76.postgres_relation.v1",
                "profile": profile,
                "sample": sample,
                "phase": phase,
                "record_count_per_family": count,
                "relation": name,
                "heap_bytes": int(heap),
                "index_bytes": int(indexes),
                "total_bytes": int(total),
            }
        )
    return rows


def timed_sql(sql: str) -> float:
    started = time.perf_counter()
    psql(sql)
    return (time.perf_counter() - started) * 1000


def explain_execution_ms(sql: str) -> float:
    plan = json.loads(psql(f"EXPLAIN (ANALYZE, FORMAT JSON) {sql}", tuples=True))
    return float(plan[0]["Execution Time"])


def wal_lsn() -> str:
    return psql("SELECT pg_current_wal_lsn();", tuples=True).strip()


def wal_delta_bytes(start_lsn: str, end_lsn: str) -> int:
    return int(
        float(
            psql(
                f"SELECT pg_wal_lsn_diff('{end_lsn}'::pg_lsn, '{start_lsn}'::pg_lsn);",
                tuples=True,
            ).strip()
        )
    )


def setup_schema() -> None:
    psql(
        """
DROP SCHEMA IF EXISTS issue76 CASCADE;
CREATE SCHEMA issue76;
CREATE TABLE issue76.revision_envelopes_compressible (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  record_id bigint NOT NULL,
  project_sequence bigint NOT NULL,
  digest bytea NOT NULL,
  payload bytea NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, record_id)
);
CREATE INDEX revision_sequence_idx
  ON issue76.revision_envelopes_compressible (owner_user_id, project_id, project_sequence);
CREATE TABLE issue76.revision_envelopes_low_compressibility
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.receipt_envelopes_compressible
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.receipt_envelopes_low_compressibility
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.activity_event_envelopes_compressible
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.activity_event_envelopes_low_compressibility
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.application_wire_records_compressible
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.application_wire_records_low_compressibility
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.payload_shape_probe_compressible
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.payload_shape_probe_low_compressibility
  (LIKE issue76.revision_envelopes_compressible INCLUDING ALL);
CREATE TABLE issue76.event_payloads (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  record_id bigint NOT NULL,
  payload bytea NOT NULL,
  digest bytea NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, record_id)
);
CREATE TABLE issue76.compaction_floors (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  record_id bigint NOT NULL,
  digest bytea NOT NULL,
  availability_gap boolean NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, record_id)
);
CREATE TABLE issue76.run_checkpoints (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  checkpoint_sequence bigint NOT NULL,
  payload bytea NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, checkpoint_sequence)
);
"""
    )


def insert_family(
    table: str,
    count: int,
    payload_bytes: int,
    offset: int,
    payload_shape: str,
) -> float:
    if payload_shape == "compressible":
        source = f"""
SELECT g,
       convert_to(repeat(chr(65 + ((g + {offset}) % 26)), {payload_bytes}), 'UTF8')
         AS payload_value
FROM generate_series(1, {count}) AS g
"""
    else:
        random_chunks = []
        remaining = payload_bytes
        while remaining:
            chunk_bytes = min(remaining, 1024)
            random_chunks.append(f"gen_random_bytes({chunk_bytes})")
            remaining -= chunk_bytes
        random_payload = " || ".join(random_chunks)
        source = f"""
SELECT g,
       {random_payload} AS payload_value
FROM generate_series(1, {count}) AS g
"""
    return timed_sql(
        f"""
WITH generated AS (
{source}
)
INSERT INTO issue76.{table}
SELECT '00000000-0000-7000-8000-000000000076'::uuid,
       '00000000-0000-7000-8000-000000007601'::uuid,
       g,
       g,
       digest(payload_value, 'sha256'),
       payload_value
FROM generated;
"""
    )


def run_profile(
    profile: dict[str, object], sample: int
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    global CONTAINER
    safe_profile = str(profile["name"]).replace("_", "-")
    CONTAINER = f"storyos-issue76-pg-{safe_profile}-{sample}"
    docker("rm", "-f", CONTAINER, check=False)
    docker(
        "run",
        "-d",
        "--name",
        CONTAINER,
        "--label",
        "storyos.issue76=true",
        "--cpus",
        str(profile["cpus"]),
        "--memory",
        str(profile["memory"]),
        "-e",
        f"POSTGRES_PASSWORD={PASSWORD}",
        IMAGE,
        "-c",
        "shared_buffers=256MB",
        "-c",
        "fsync=on",
        "-c",
        "full_page_writes=on",
        "-c",
        "synchronous_commit=on",
    )
    wait_ready()
    psql("CREATE EXTENSION IF NOT EXISTS pgcrypto;", database="postgres")
    psql("DROP DATABASE IF EXISTS storyos_issue76;", database="postgres")
    psql("CREATE DATABASE storyos_issue76;", database="postgres")
    psql("CREATE EXTENSION IF NOT EXISTS pgcrypto;")

    relation_rows: list[dict[str, object]] = []
    operations: list[dict[str, object]] = []
    for count in WORKLOAD["postgres"]["record_counts"]:
        setup_schema()
        load_start_lsn = wal_lsn()
        payloads = WORKLOAD["postgres"]["payload_bytes"]
        family_specs = [
            ("revision_envelopes", payloads["revision"]),
            ("receipt_envelopes", payloads["receipt"]),
            ("activity_event_envelopes", payloads["activity_event"]),
            ("application_wire_records", payloads["application_wire_record"]),
        ]
        for offset, (family, payload_bytes) in enumerate(family_specs):
            for payload_shape in ("compressible", "low_compressibility"):
                table = f"{family}_{payload_shape}"
                elapsed = insert_family(table, count, payload_bytes, offset, payload_shape)
                operations.append(
                    {
                        "schema": "storyos.issue76.postgres_operation.v1",
                        "profile": profile["name"],
                        "sample": sample,
                        "operation": "insert_family",
                        "relation": table,
                        "record_family": family,
                        "payload_shape": payload_shape,
                        "record_count": count,
                        "payload_bytes": payload_bytes,
                        "elapsed_ms": elapsed,
                    }
                )
        if count == 10000:
            for payload_shape in ("compressible", "low_compressibility"):
                table = f"payload_shape_probe_{payload_shape}"
                elapsed = insert_family(
                    table,
                    count,
                    payloads["payload_shape_probe"],
                    8,
                    payload_shape,
                )
                operations.append(
                    {
                        "schema": "storyos.issue76.postgres_operation.v1",
                        "profile": profile["name"],
                        "sample": sample,
                        "operation": "payload_shape_probe",
                        "relation": table,
                        "payload_shape": payload_shape,
                        "record_count": count,
                        "payload_bytes": payloads["payload_shape_probe"],
                        "elapsed_ms": elapsed,
                    }
                )
        psql("CHECKPOINT;")
        load_end_lsn = wal_lsn()
        operations.append(
            {
                "schema": "storyos.issue76.postgres_operation.v1",
                "profile": profile["name"],
                "sample": sample,
                "operation": "loaded_wal_growth",
                "record_count_per_family": count,
                "wal_start_lsn": load_start_lsn,
                "wal_end_lsn": load_end_lsn,
                "wal_delta_bytes": wal_delta_bytes(load_start_lsn, load_end_lsn),
            }
        )
        relation_rows.extend(
            relation_metrics(str(profile["name"]), sample, "loaded", count)
        )

    count = max(WORKLOAD["postgres"]["record_counts"])
    setup_schema()
    payloads = WORKLOAD["postgres"]["payload_bytes"]
    compaction_load_start_lsn = wal_lsn()
    insert_family(
        "activity_event_envelopes_low_compressibility",
        count,
        payloads["activity_event"],
        2,
        "low_compressibility",
    )
    timed_sql(
        f"""
WITH generated AS (
  SELECT g,
         gen_random_bytes(1024) || gen_random_bytes(1024) AS payload_value
  FROM generate_series(1, {count}) AS g
)
INSERT INTO issue76.event_payloads
SELECT '00000000-0000-7000-8000-000000000076'::uuid,
       '00000000-0000-7000-8000-000000007601'::uuid,
       g,
       substring(payload_value for {payloads["compactable_event_payload"]}),
       digest(substring(payload_value for {payloads["compactable_event_payload"]}), 'sha256')
FROM generated;
"""
    )
    compaction_load_end_lsn = wal_lsn()
    relation_rows.extend(
        relation_metrics(str(profile["name"]), sample, "pre_compaction", count)
    )
    keep_every = round(1 / WORKLOAD["postgres"]["compaction_keep_fraction"])
    compaction_start_lsn = wal_lsn()
    compaction_ms = timed_sql(
        f"""
INSERT INTO issue76.compaction_floors
SELECT owner_user_id, project_id, record_id, digest, true
FROM issue76.event_payloads
WHERE record_id % {keep_every} <> 0;
DELETE FROM issue76.event_payloads WHERE record_id % {keep_every} <> 0;
"""
    )
    psql("VACUUM (ANALYZE) issue76.event_payloads;")
    relation_rows.extend(
        relation_metrics(str(profile["name"]), sample, "post_delete_vacuum", count)
    )
    vacuum_full_ms = timed_sql("VACUUM FULL issue76.event_payloads;")
    compaction_end_lsn = wal_lsn()
    relation_rows.extend(
        relation_metrics(str(profile["name"]), sample, "post_vacuum_full", count)
    )
    operations.append(
        {
            "schema": "storyos.issue76.postgres_operation.v1",
            "profile": profile["name"],
            "sample": sample,
            "operation": "compaction",
            "record_count": count,
            "keep_fraction": WORKLOAD["postgres"]["compaction_keep_fraction"],
            "delete_and_floor_ms": compaction_ms,
            "vacuum_full_ms": vacuum_full_ms,
            "load_wal_delta_bytes": wal_delta_bytes(
                compaction_load_start_lsn, compaction_load_end_lsn
            ),
            "compaction_wal_delta_bytes": wal_delta_bytes(
                compaction_start_lsn, compaction_end_lsn
            ),
        }
    )

    checkpoint_bytes = payloads["checkpoint"]
    psql(
        f"""
INSERT INTO issue76.run_checkpoints
VALUES (
  '00000000-0000-7000-8000-000000000076'::uuid,
  '00000000-0000-7000-8000-000000007601'::uuid,
  {count - WORKLOAD["postgres"]["replay_events_after_checkpoint"]},
  decode(repeat('ab', {checkpoint_bytes}), 'hex')
);
"""
    )
    full_replay_ms = explain_execution_ms(
        "SELECT count(*), sum(project_sequence) "
        "FROM issue76.activity_event_envelopes_low_compressibility;"
    )
    bounded_replay_ms = explain_execution_ms(
        f"""SELECT count(*), sum(project_sequence)
FROM issue76.activity_event_envelopes_low_compressibility
WHERE project_sequence > {count - WORKLOAD["postgres"]["replay_events_after_checkpoint"]};"""
    )
    operations.append(
        {
            "schema": "storyos.issue76.postgres_operation.v1",
            "profile": profile["name"],
            "sample": sample,
            "operation": "checkpoint_replay_scan",
            "events_total": count,
            "events_after_checkpoint": WORKLOAD["postgres"]["replay_events_after_checkpoint"],
            "full_replay_ms": full_replay_ms,
            "bounded_replay_ms": bounded_replay_ms,
        }
    )

    dump_in_container = "/tmp/storyos_issue76.dump"
    started = time.perf_counter()
    docker("exec", CONTAINER, "pg_dump", "-U", "postgres", "-Fc", "-f", dump_in_container, "storyos_issue76")
    dump_ms = (time.perf_counter() - started) * 1000
    dump_size = int(docker("exec", CONTAINER, "stat", "-c", "%s", dump_in_container).stdout.strip())
    psql("DROP DATABASE IF EXISTS storyos_issue76_restore;", database="postgres")
    psql("CREATE DATABASE storyos_issue76_restore;", database="postgres")
    started = time.perf_counter()
    docker(
        "exec",
        CONTAINER,
        "pg_restore",
        "-U",
        "postgres",
        "-d",
        "storyos_issue76_restore",
        dump_in_container,
    )
    restore_ms = (time.perf_counter() - started) * 1000
    source_digest = psql(
        "SELECT count(*), sum(project_sequence) "
        "FROM issue76.activity_event_envelopes_low_compressibility;",
        tuples=True,
    ).strip()
    restored_digest = psql(
        "SELECT count(*), sum(project_sequence) "
        "FROM issue76.activity_event_envelopes_low_compressibility;",
        database="storyos_issue76_restore",
        tuples=True,
    ).strip()
    operations.append(
        {
            "schema": "storyos.issue76.postgres_operation.v1",
            "profile": profile["name"],
            "sample": sample,
            "operation": "logical_backup_restore",
            "dump_bytes": dump_size,
            "dump_ms": dump_ms,
            "restore_ms": restore_ms,
            "source_count_and_sequence_sum": source_digest,
            "restored_count_and_sequence_sum": restored_digest,
            "validated_equal": source_digest == restored_digest,
            "source_database_bytes": int(
                psql("SELECT pg_database_size(current_database());", tuples=True).strip()
            ),
            "scope_limit": "logical synthetic path; not physical base backup, WAL, RPO, RTO, or Recovery Visibility Proof",
        }
    )
    docker("rm", "-f", CONTAINER, check=False)
    return relation_rows, operations


def main() -> int:
    all_relations: list[dict[str, object]] = []
    all_operations: list[dict[str, object]] = []
    try:
        for profile in WORKLOAD["postgres"]["profiles"]:
            for sample in range(WORKLOAD["postgres"]["measured_runs"]):
                relations, operations = run_profile(profile, sample)
                all_relations.extend(relations)
                all_operations.extend(operations)
    finally:
        cleanup_containers()

    relation_path = OUT / "postgres-relation-growth.csv"
    with relation_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=list(all_relations[0]),
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(all_relations)
    operation_path = OUT / "postgres-operations.jsonl"
    operation_path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in all_operations),
        encoding="utf-8",
    )
    environment = {
        "schema": "storyos.issue76.environment.v1",
        "python": sys.version,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "host_cpu": run(["sysctl", "-n", "machdep.cpu.brand_string"]).stdout.strip(),
        "host_logical_cpus": int(run(["sysctl", "-n", "hw.ncpu"]).stdout),
        "host_memory_bytes": int(run(["sysctl", "-n", "hw.memsize"]).stdout),
        "docker_server": docker("version", "--format", "{{.Server.Version}}").stdout.strip(),
        "postgres_image": IMAGE,
        "postgres_image_id": docker("image", "inspect", IMAGE, "--format", "{{.Id}}").stdout.strip(),
        "postgres_version": docker("run", "--rm", IMAGE, "postgres", "--version").stdout.strip(),
        "workload_sha256": hashlib.sha256((HERE / "workload.json").read_bytes()).hexdigest(),
        "apparatus_process_max_rss_bytes": resource.getrusage(
            resource.RUSAGE_SELF
        ).ru_maxrss,
        "note": "Host serial numbers, usernames, and credential material intentionally excluded.",
    }
    (OUT / "environment.json").write_text(json.dumps(environment, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"relations": len(all_relations), "operations": len(all_operations)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
