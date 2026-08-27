"""Storage-side validation for the Release 1 PostgreSQL persistence catalog.

This module owns migration/bootstrap, family/scope/projection/portability, and
the physical-ledger boundary.  It does not validate public route coverage; the
route module consumes the ownership maps returned by ``validate_families``.
"""

from __future__ import annotations

import hashlib
import re
from collections import defaultdict
from typing import Any

from postgresql_persistence_verifier_common import (
    ACTIVE_SCHEMA,
    ALLOWED_AUTHORITY_CLASSES,
    ALLOWED_PORTABILITY,
    ALLOWED_PROJECTION_KINDS,
    ALLOWED_SETTLEMENT_MODES,
    ALLOWED_SCOPE_KINDS,
    ALLOWED_WIRE_HISTORY,
    PROJECT_SCOPE_PROFILE,
    ROOT,
    STORAGE_CONTRACT_PATH,
    canonical_bytes,
    dependency_path_count,
    fail,
    graph_has_cycle,
    normalized_file_bytes,
    selector_events,
    selector_operations,
    validate_source,
)


def validate_migration(catalog: dict[str, Any], errors: list[str]) -> None:
    identity = catalog.get("schema_identity", {})
    migration = catalog.get("migration_chain", {})
    if identity.get("database_schema_identity") != "storyos.postgresql.schema.v3":
        fail(errors, "database_schema_identity must be the hard-cut PostgreSQL schema v3")
    if identity.get("active_schema_version") != ACTIVE_SCHEMA:
        fail(errors, "schema_identity.active_schema_version must be the Release 1 schema")
    if identity.get("persisted_format_catalog_id") != catalog.get("catalog_id"):
        fail(errors, "persisted_format_catalog_id must equal catalog_id")
    if identity.get("migration_chain_id") != migration.get("chain_id"):
        fail(errors, "schema identity and migration chain identities disagree")
    if migration.get("chain_id") != "storyos.persistence.bootstrap.release-1.v3":
        fail(errors, "migration chain must be the hard-cut Release 1 bootstrap v3")
    if migration.get("current_version") != ACTIVE_SCHEMA:
        fail(errors, "migration_chain.current_version must be the active schema")
    predecessors = migration.get("supported_predecessors")
    edges = migration.get("edges")
    if (
        not isinstance(predecessors, list)
        or any(not isinstance(item, str) for item in predecessors)
        or len(predecessors) != len(set(predecessors))
    ):
        fail(errors, "migration_chain.supported_predecessors must be a unique string list")
        predecessors = []
    if not isinstance(edges, list):
        fail(errors, "migration_chain.edges must be an array")
        edges = []
    if not predecessors and edges:
        fail(errors, "initial Release 1 baseline cannot declare migration edges without a predecessor")
    if predecessors and not edges:
        fail(errors, "supported migration predecessors require at least one migration edge")
    edge_keys: set[tuple[str, str]] = set()
    migration_ids: set[str] = set()
    graph: dict[str, list[str]] = defaultdict(list)
    for edge in edges:
        if not isinstance(edge, dict):
            fail(errors, "migration edge must be an object")
            continue
        key = (edge.get("from"), edge.get("to"))
        if key in edge_keys:
            fail(errors, f"duplicate migration edge {key}")
        edge_keys.add(key)
        migration_id = edge.get("migration_id")
        if not isinstance(migration_id, str) or not migration_id:
            fail(errors, f"migration edge has no stable migration_id: {edge}")
        elif migration_id in migration_ids:
            fail(errors, f"duplicate migration_id {migration_id}")
        else:
            migration_ids.add(migration_id)
        if not all(isinstance(value, str) for value in key):
            fail(errors, f"migration edge has invalid endpoints {key}")
            continue
        graph[key[0]].append(key[1])
        if edge.get("to") != ACTIVE_SCHEMA:
            fail(errors, f"migration edge does not terminate at active schema: {edge}")
        if edge.get("unique_path") is not True:
            fail(errors, f"migration edge must declare unique_path: {edge.get('migration_id')}")
        if not edge.get("phase_ids"):
            fail(errors, f"migration edge has no phases: {edge.get('migration_id')}")
    for predecessor in predecessors:
        if dependency_path_count(graph, predecessor, ACTIVE_SCHEMA) != 1:
            fail(errors, f"supported predecessor does not have exactly one acyclic path: {predecessor}")
    if graph_has_cycle(graph):
        fail(errors, "migration graph contains a cycle")
    if catalog.get("verification_contract", {}).get("require_empty_edge_set_for_initial_baseline") is True:
        if predecessors or edges:
            fail(errors, "initial Release 1 baseline must have an empty predecessor and edge set")
        if migration.get("new_install_terminal") != ACTIVE_SCHEMA:
            fail(errors, "initial Release 1 baseline must terminate new installation at the active schema")
    validate_bootstrap_sources(migration, errors)
    phases = catalog.get("bootstrap_phases")
    if not isinstance(phases, list) or not phases:
        fail(errors, "bootstrap_phases must be non-empty")
        phases = []
    phase_ids = [phase.get("phase_id") for phase in phases if isinstance(phase, dict)]
    if len(phase_ids) != len(set(phase_ids)):
        fail(errors, "bootstrap phase IDs must be unique")
    phase_map = {phase.get("phase_id"): phase for phase in phases if isinstance(phase, dict)}
    required_phase_ids = {"preflight", "initial_schema_bootstrap", "validate_transaction", "activate"}
    if set(phase_map) != required_phase_ids:
        fail(errors, f"bootstrap phase catalog does not equal initial baseline phase set: {set(phase_map) ^ required_phase_ids}")
    for phase_id, phase in phase_map.items():
        for field in ("class", "transactional", "restartable", "idempotent", "postcondition", "failure_state"):
            if field not in phase:
                fail(errors, f"bootstrap phase {phase_id} missing {field}")
        for field in ("transactional", "restartable", "idempotent"):
            if not isinstance(phase.get(field), bool):
                fail(errors, f"bootstrap phase {phase_id}.{field} must be boolean")
    if catalog.get("migration_phases"):
        fail(errors, "initial Release 1 catalog must not claim a predecessor migration phase list")
    state_machine = catalog.get("state_machine", {})
    states = state_machine.get("states", [])
    transitions = state_machine.get("transitions", [])
    if not states or len(states) != len(set(states)):
        fail(errors, "state machine states must be non-empty and unique")
    state_set = set(states)
    transition_pairs: dict[str, list[str]] = defaultdict(list)
    for transition in transitions:
        if not isinstance(transition, dict):
            fail(errors, "state transition must be an object")
            continue
        if transition.get("from") not in state_set or transition.get("to") not in state_set:
            fail(errors, f"state transition references unknown state: {transition}")
        transition_pairs[transition.get("from")].append(transition.get("to"))
    if state_machine.get("terminal_active_state") != "active":
        fail(errors, "state machine terminal active state must be active")
    if dependency_path_count(transition_pairs, "new_install_required", "active") == 0:
        fail(errors, "new-install state has no positive path to active")
    forbidden_initial_states = {"predecessor_present", "recovery_copy_required", "recovery_copy_verified", "migrating", "contract_cleanup_pending"}
    if state_set & forbidden_initial_states:
        fail(errors, f"initial Release 1 state machine invents predecessor/migration states: {sorted(state_set & forbidden_initial_states)}")
    active_gates = state_machine.get("active_write_gates", [])
    required_gates = {
        "same_release_identity_exact",
        "migration_chain_complete",
        "initial_schema_bootstrap_complete",
        "bootstrap_restore_proof_current",
        "recovery_visibility_proof_current",
    }
    if not required_gates <= set(active_gates):
        fail(errors, "active write gates omit initial bootstrap, release, migration, or recovery visibility proof")
    validate_author_edit_receipt_activity(catalog, errors)
    validate_author_command_outcome_unknown(catalog, errors)
    validate_takeover_receipt_activity(catalog, errors)


def validate_bootstrap_sources(migration: dict[str, Any], errors: list[str]) -> None:
    bootstrap = migration.get("bootstrap", {})
    expected_paths = [
        "crates/storyos-adapter-postgres/migrations/0000_roles.sql",
        "crates/storyos-adapter-postgres/migrations/0001_controlled_project.sql",
        "crates/storyos-adapter-postgres/migrations/0002_project_command_challenges.sql",
        "crates/storyos-adapter-postgres/migrations/0004_editor_sessions.sql",
        "crates/storyos-adapter-postgres/migrations/0005_author_edits.sql",
        "crates/storyos-adapter-postgres/migrations/0006_snapshot_replay.sql",
        "crates/storyos-adapter-postgres/migrations/0007_takeover_admission_activity.sql",
        "crates/storyos-adapter-postgres/migrations/0008_create_project_challenge.sql",
        "crates/storyos-adapter-postgres/migrations/0009_create_empty_project.sql",
        "crates/storyos-adapter-postgres/migrations/0010_list_owned_projects.sql",
        "crates/storyos-adapter-postgres/migrations/0011_update_project.sql",
        "crates/storyos-adapter-postgres/migrations/0012_archive_project.sql",
        "crates/storyos-adapter-postgres/migrations/0013_manuscript_tree.sql",
        "crates/storyos-adapter-postgres/migrations/0014_create_volume.sql",
        "crates/storyos-adapter-postgres/migrations/0015_create_chapter.sql",
    ]
    if bootstrap.get("transaction_boundary") != "one_postgresql_transaction":
        fail(errors, "Release 1 bootstrap must use one PostgreSQL transaction")
    if bootstrap.get("digest_profile") != "storyos.persistence.bootstrap-manifest-jcs.v1":
        fail(errors, "Release 1 bootstrap digest profile is missing or incorrect")
    if bootstrap.get("runtime_credential_source") != "external_secret_after_atomic_bootstrap":
        fail(errors, "Release 1 bootstrap must not own the runtime credential")
    sources = bootstrap.get("sources")
    if not isinstance(sources, list):
        fail(errors, "Release 1 bootstrap sources must be an ordered array")
        return
    paths = [source.get("path") for source in sources if isinstance(source, dict)]
    if paths != expected_paths or len(paths) != len(sources):
        fail(errors, "Release 1 bootstrap sources do not equal the closed active SQL set")
        return
    migration_dir = ROOT / "crates/storyos-adapter-postgres/migrations"
    checked_in_paths = sorted(
        path.relative_to(ROOT).as_posix() for path in migration_dir.glob("*.sql")
    )
    if checked_in_paths != expected_paths:
        fail(errors, "checked-in SQL sources do not equal the catalogued bootstrap set")
    actual_manifest: list[dict[str, str]] = []
    for source in sources:
        source_path = ROOT / source["path"]
        if not source_path.is_file():
            fail(errors, f"missing bootstrap SQL source {source['path']}")
            continue
        normalized = normalized_file_bytes(source_path)
        actual_digest = "sha256:" + hashlib.sha256(normalized).hexdigest()
        if source.get("lf_sha256") != actual_digest:
            fail(errors, f"bootstrap SQL checksum mismatch for {source['path']}: expected {actual_digest}")
        if re.search(rb"(?im)^\s*(?:BEGIN|COMMIT)\s*;\s*$", normalized):
            fail(errors, f"bootstrap SQL source contains an inner transaction boundary: {source['path']}")
        if source["path"].endswith("/0000_roles.sql") and re.search(
            rb"(?i)\bPASSWORD\b", normalized
        ):
            fail(errors, "Release 1 role bootstrap contains tracked password material")
        actual_manifest.append({"path": source["path"], "lf_sha256": actual_digest})
    manifest_digest = "sha256:" + hashlib.sha256(canonical_bytes(actual_manifest)).hexdigest()
    if bootstrap.get("manifest_sha256") != manifest_digest:
        fail(errors, f"bootstrap manifest checksum mismatch: expected {manifest_digest}")


def validate_author_edit_receipt_activity(
    catalog: dict[str, Any], errors: list[str]
) -> None:
    contract = catalog.get("author_edit_receipt_activity", {})
    expected_scalars = {
        "schema_id": "storyos.persistence.author-edit-receipt-activity.v1",
        "receipt_table_family": "domain_receipts",
        "activity_table_family": "project_activity_events",
        "receipt_identity": "independent_immutable_typed",
        "applied_result_kind": "authoritative_applied",
        "applied_relation_cardinality": "exactly_one_complete_activity_authority_relation",
        "authority_relation_binding": "composite_applied_receipt_admission_object_prior_revision",
        "zero_relation_cardinality": "no_activity_revision_payload_commit_head_or_author_action",
        "typed_value_guard": "one_based_single_identity_arrays_no_sql_or_json_null",
    }
    for field, expected in expected_scalars.items():
        if contract.get(field) != expected:
            fail(errors, f"author-edit Receipt/Activity contract has invalid {field}")
    expected_lists = {
        "zero_authority_result_kinds": ["no_effect", "conflicted", "refused"],
        "zero_settlement_write_set": [
            "typed_domain_receipt",
            "admission_receipt_settled",
            "idempotency_result_reference",
        ],
        "exact_retry_order": [
            "read_idempotency_result_reference",
            "read_receipt_by_receipt_id",
            "validate_admission_target_and_expected_head",
            "validate_receipt_heads_in_target",
            "branch_on_receipt_result_kind",
            "validate_result_reason_and_head_semantics",
            "validate_exact_authority_activity_cardinality_and_binding",
            "validate_applied_prior_and_payload_digest",
            "return_original_settlement",
        ],
        "prohibited_representation": [
            "nullable_or_sentinel_authority",
            "fake_activity",
            "shadow_or_zero_receipt_table",
            "dual_read_or_dual_write",
            "compatibility_view_or_wrapper",
        ],
    }
    for field, expected in expected_lists.items():
        if contract.get(field) != expected:
            fail(errors, f"author-edit Receipt/Activity contract has invalid {field}")


def validate_takeover_receipt_activity(
    catalog: dict[str, Any], errors: list[str]
) -> None:
    contract = catalog.get("takeover_receipt_activity", {})
    expected = {
        "schema_id": "storyos.persistence.takeover-receipt-activity.v1",
        "receipt_table_family": "domain_receipts",
        "activity_table_family": "project_activity_event_payloads",
        "command_kind": "takeOverProjectWriter",
        "action_class": "explicit_editor_command",
        "result_kind": "no_effect",
        "writer_generation": "append_only_history",
        "author_edit_zero_authority": "unchanged_no_activity",
    }
    for field, expected_value in expected.items():
        if contract.get(field) != expected_value:
            fail(errors, f"takeover Receipt/Activity contract has invalid {field}")
    if contract.get("event_kinds") != [
        "writer_takeover_applied",
        "writer_takeover_compare_failed",
    ]:
        fail(errors, "takeover Receipt/Activity contract has invalid event_kinds")
    if contract.get("prohibited_representation") != [
        "nullable_or_sentinel_authority",
        "fake_activity",
        "shadow_or_zero_receipt_table",
        "dual_read_or_dual_write",
        "compatibility_view_or_wrapper",
        "update_or_delete_writer_generation",
    ]:
        fail(errors, "takeover Receipt/Activity contract has invalid prohibited_representation")
    family = next(
        (
            family
            for family in catalog.get("families", [])
            if isinstance(family, dict)
            and family.get("family_id") == "operational-project-activity"
        ),
        {},
    )
    if expected["activity_table_family"] not in family.get("table_families", []):
        fail(errors, "takeover Activity table must belong to operational-project-activity")
    sql_path = ROOT / "crates/storyos-adapter-postgres/migrations/0007_takeover_admission_activity.sql"
    sql = normalized_file_bytes(sql_path).decode("utf-8")
    required_fragments = [
        "PRIMARY KEY (owner_user_id, project_id, writer_generation)",
        "command_kind = 'takeOverProjectWriter'",
        "action_class = 'explicit_editor_command'",
        "CREATE TABLE storyos.project_activity_event_payloads",
        "writer_takeover_applied",
        "writer_takeover_compare_failed",
        "GRANT SELECT, INSERT ON storyos.project_activity_event_payloads TO storyos_runtime;",
    ]
    for fragment in required_fragments:
        if fragment not in sql:
            fail(errors, f"takeover bootstrap is missing locked SQL: {fragment}")
    if re.search(r"GRANT\s+UPDATE\s+ON\s+storyos\.project_writer_generations", sql, flags=re.I):
        fail(errors, "takeover bootstrap must not grant UPDATE on writer generations")


def validate_author_command_outcome_unknown(
    catalog: dict[str, Any], errors: list[str]
) -> None:
    expected = {
        "schema_id": "storyos.persistence.author-command-outcome-unknown.v1",
        "table_family": "author_command_admission_outcome_unknown_observations",
        "owner_family_id": "operational-admission-editor",
        "row_identity": ["owner_user_id", "project_id", "observation_id"],
        "sequence_identity": [
            "owner_user_id",
            "project_id",
            "author_command_admission_id",
            "observation_sequence",
        ],
        "append_request_fields": [
            "project_scope",
            "author_command_admission_id",
            "observation_id",
            "last_provable_boundary",
            "reason",
        ],
        "copied_admission_identity": [
            "command_id",
            "command_kind",
            "idempotency_key",
            "canonical_command_digest",
        ],
        "last_provable_boundaries": ["admission_committed"],
        "reasons": ["acknowledgement_missing"],
        "reconciliation_required": "literal_true",
        "observed_at": "postgresql_clock",
        "sequence": "one_based_u64_database_serialized",
        "arbiter": "same_scoped_command_idempotency_row_as_receipt_settlement",
        "open_condition": "idempotency_in_progress_and_no_terminal_settlement",
        "current_lifecycle_projection": {
            "pending": "no_terminal_and_zero_observations",
            "outcome_unknown": "no_terminal_and_one_or_more_observations",
            "receipt_settled": "terminal_precedes_historical_observations",
        },
        "exact_retry": "same_complete_scoped_observation_returns_original_row_and_time_even_after_terminal",
        "new_after_terminal": "reject",
        "new_after_sequence_exhaustion": "reject_atomically",
        "runtime_grants": ["select", "insert"],
        "prohibited_effects": [
            "update_or_delete_observation",
            "command_idempotency_update",
            "challenge_consumption",
            "core_invocation",
            "receipt_or_settlement_creation",
            "activity_or_authority_creation",
            "retry_permission",
            "success_or_rejection_projection",
        ],
    }
    if catalog.get("author_command_outcome_unknown") != expected:
        fail(errors, "author-command outcome_unknown contract must equal the closed v1 shape")

    family = next(
        (
            family
            for family in catalog.get("families", [])
            if isinstance(family, dict)
            and family.get("family_id") == "operational-admission-editor"
        ),
        {},
    )
    if expected["table_family"] not in family.get("table_families", []):
        fail(errors, "outcome_unknown table must belong to operational-admission-editor")

    sql_path = ROOT / "crates/storyos-adapter-postgres/migrations/0005_author_edits.sql"
    sql = normalized_file_bytes(sql_path).decode("utf-8")
    validate_author_command_outcome_unknown_sql(sql, errors)


def validate_author_command_outcome_unknown_sql(sql: str, errors: list[str]) -> None:
    required_fragments = [
        "CREATE TABLE storyos.author_command_admission_outcome_unknown_observations (",
        "observation_id uuid NOT NULL",
        "observation_sequence numeric(20, 0) NOT NULL",
        "PRIMARY KEY (owner_user_id, project_id, observation_id)",
        "UNIQUE (owner_user_id, project_id, author_command_admission_id, observation_sequence)",
        "CHECK (observation_sequence BETWEEN 1 AND 18446744073709551615)",
        "last_provable_boundary text NOT NULL\n    CHECK (last_provable_boundary = 'admission_committed')",
        "reason text NOT NULL CHECK (reason = 'acknowledgement_missing')",
        "reconciliation_required boolean NOT NULL DEFAULT true\n    CHECK (reconciliation_required)",
        "observed_at timestamptz NOT NULL DEFAULT clock_timestamp()",
        "FOREIGN KEY (\n    owner_user_id, project_id, author_command_admission_id, command_id,\n    command_kind, canonical_command_digest, idempotency_key\n  ) REFERENCES storyos.author_command_admissions (\n    owner_user_id, project_id, author_command_admission_id, command_id,\n    command_kind, canonical_command_digest, idempotency_key\n  ) MATCH FULL",
        "CREATE TRIGGER author_command_outcome_unknown_prepare",
        "BEFORE INSERT ON storyos.author_command_admission_outcome_unknown_observations",
        "NEW.command_id := admission.command_id;",
        "NEW.command_kind := admission.command_kind;",
        "NEW.canonical_command_digest := admission.canonical_command_digest;",
        "NEW.idempotency_key := admission.idempotency_key;",
        "NEW.observation_sequence := next_sequence;",
        "NEW.observed_at := clock_timestamp();",
        "ALTER TABLE storyos.author_command_admission_outcome_unknown_observations\n  ENABLE ROW LEVEL SECURITY;",
        "ALTER TABLE storyos.author_command_admission_outcome_unknown_observations\n  FORCE ROW LEVEL SECURITY;",
        "CREATE POLICY author_command_outcome_unknown_exact_scope",
        "storyos.author_command_admission_outcome_unknown_observations,",
    ]
    for fragment in required_fragments:
        if fragment not in sql:
            fail(errors, f"outcome_unknown bootstrap is missing locked SQL: {fragment}")
    if not re.search(
        r"SELECT\s+command_id,\s*command_kind,\s*canonical_command_digest,\s*"
        r"idempotency_key\s+INTO\s+admission\s+"
        r"FROM\s+storyos\.author_command_admissions\s+"
        r"WHERE\s+owner_user_id\s*=\s*NEW\.owner_user_id\s+"
        r"AND\s+project_id\s*=\s*NEW\.project_id\s+"
        r"AND\s+author_command_admission_id\s*=\s*NEW\.author_command_admission_id\s*;",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        fail(errors, "outcome_unknown append must load one exact scoped Admission")
    if not re.search(
        r"SELECT\s+outcome_kind\s+INTO\s+arbiter_outcome_kind\s+"
        r"FROM\s+storyos\.command_idempotency\s+"
        r"WHERE\s+owner_user_id\s*=\s*NEW\.owner_user_id\s+"
        r"AND\s+project_id\s*=\s*NEW\.project_id\s+"
        r"AND\s+command_kind\s*=\s*admission\.command_kind\s+"
        r"AND\s+idempotency_key\s*=\s*admission\.idempotency_key\s+"
        r"FOR\s+UPDATE\s*;",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        fail(errors, "outcome_unknown append must lock the exact scoped idempotency arbiter")
    if not re.search(
        r"IF\s+NOT\s+FOUND\s+OR\s+arbiter_outcome_kind\s*<>\s*'in_progress'\s+THEN",
        sql,
        flags=re.IGNORECASE,
    ):
        fail(errors, "outcome_unknown append must require the in-progress arbiter state")
    if not re.search(
        r"IF\s+EXISTS\s*\(\s*SELECT\s+1\s+"
        r"FROM\s+storyos\.author_command_admission_settlements\s+"
        r"WHERE\s+owner_user_id\s*=\s*NEW\.owner_user_id\s+"
        r"AND\s+project_id\s*=\s*NEW\.project_id\s+"
        r"AND\s+author_command_admission_id\s*=\s*NEW\.author_command_admission_id\s*"
        r"\)\s+THEN",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        fail(errors, "outcome_unknown append must reject a terminal Admission")
    if not re.search(
        r"SELECT\s+COALESCE\(max\(observation_sequence\),\s*0\)\s*\+\s*1\s+"
        r"INTO\s+next_sequence\s+"
        r"FROM\s+storyos\.author_command_admission_outcome_unknown_observations\s+"
        r"WHERE\s+owner_user_id\s*=\s*NEW\.owner_user_id\s+"
        r"AND\s+project_id\s*=\s*NEW\.project_id\s+"
        r"AND\s+author_command_admission_id\s*=\s*NEW\.author_command_admission_id\s*;",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        fail(errors, "outcome_unknown sequence must be database-allocated within one Admission")
    scope_expression = (
        r"owner_user_id\s*=\s*current_setting\('storyos\.owner_user_id'\)::uuid\s+"
        r"AND\s+project_id\s*=\s*current_setting\('storyos\.project_id'\)::uuid"
    )
    if not re.search(
        r"CREATE\s+POLICY\s+author_command_outcome_unknown_exact_scope\s+"
        r"ON\s+storyos\.author_command_admission_outcome_unknown_observations\s+"
        rf"USING\s*\(\s*{scope_expression}\s*\)\s+"
        rf"WITH\s+CHECK\s*\(\s*{scope_expression}\s*\)\s*;",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        fail(errors, "outcome_unknown table needs exact User and Project Scope RLS")
    if not re.search(
        r"GRANT\s+SELECT,\s*INSERT\s+ON\s+[^;]*"
        r"storyos\.author_command_admission_outcome_unknown_observations[^;]*"
        r"TO\s+storyos_runtime\s*;",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        fail(errors, "outcome_unknown table needs SELECT and INSERT runtime grants")
    for grant in re.finditer(
        r"GRANT\s+(?P<privileges>[^;]+?)\s+ON\s+(?P<tables>[^;]+?)\s+"
        r"TO\s+(?P<roles>[^;]+)\s*;",
        sql,
        flags=re.IGNORECASE | re.DOTALL,
    ):
        tables = grant.group("tables")
        if "storyos.author_command_admission_outcome_unknown_observations" not in tables:
            continue
        privileges = {
            privilege.strip().upper()
            for privilege in grant.group("privileges").split(",")
        }
        if "STORYOS_RUNTIME" in grant.group("roles").upper() and privileges != {
            "SELECT",
            "INSERT",
        }:
            fail(errors, "outcome_unknown runtime grants must equal SELECT and INSERT")
        if privileges - {"SELECT", "INSERT"}:
            fail(errors, "outcome_unknown table must not grant mutation privileges")


def validate_families(catalog: dict[str, Any], route_catalog: dict[str, Any], errors: list[str]) -> tuple[dict[str, dict[str, Any]], dict[str, set[str]], dict[str, set[str]]]:
    owners = catalog.get("owners", {})
    families = catalog.get("families")
    if not isinstance(families, list) or not families:
        fail(errors, "families must be a non-empty array")
        return {}, {}, {}
    family_map: dict[str, dict[str, Any]] = {}
    table_owner: dict[str, str] = {}
    operation_owners: dict[str, set[str]] = defaultdict(set)
    event_owners: dict[str, set[str]] = defaultdict(set)
    operations = route_catalog.get("operations", [])
    events = route_catalog.get("events", [])
    operation_by_id = {operation.get("operation_id"): operation for operation in operations}
    event_by_id = {event.get("schema_id"): event for event in events}
    global_predecessors = catalog.get("migration_chain", {}).get("supported_predecessors", [])
    family_ids = {
        family.get("family_id")
        for family in families
        if isinstance(family, dict) and isinstance(family.get("family_id"), str)
    }
    for family in families:
        if not isinstance(family, dict):
            fail(errors, "persistence family must be an object")
            continue
        family_id = family.get("family_id")
        if not isinstance(family_id, str) or not family_id:
            fail(errors, "persistence family needs a stable family_id")
            continue
        if family_id in family_map:
            fail(errors, f"duplicate family_id {family_id}")
        family_map[family_id] = family
        if family.get("owner") not in owners:
            fail(errors, f"{family_id}: owner is not in owners")
        if family.get("authority_class") not in ALLOWED_AUTHORITY_CLASSES:
            fail(errors, f"{family_id}: invalid authority_class")
        if not isinstance(family.get("table_families"), list) or not family["table_families"]:
            fail(errors, f"{family_id}: table_families must be non-empty")
        for table_name in family.get("table_families", []):
            if table_name in table_owner:
                fail(errors, f"table family {table_name} is merged by {table_owner[table_name]} and {family_id}")
            table_owner[table_name] = family_id
        scope = family.get("scope", {})
        if scope.get("kind") not in ALLOWED_SCOPE_KINDS:
            fail(errors, f"{family_id}: invalid scope kind")
        if scope.get("kind") == "project" and scope.get("requires_project_scope") is not True:
            fail(errors, f"{family_id}: project family must require Project Scope")
        if scope.get("requires_project_scope") is True and scope.get("kind") != "project":
            fail(errors, f"{family_id}: only a project family may require Project Scope")
        profile_name = scope.get("profile")
        profile = catalog.get("scope_profiles", {}).get(profile_name)
        if not isinstance(profile, dict):
            fail(errors, f"{family_id}: missing scope profile {profile_name!r}")
        else:
            if profile.get("scope_kind") != scope.get("kind"):
                fail(errors, f"{family_id}: scope kind disagrees with profile")
            if scope.get("requires_project_scope") is True:
                if profile_name != PROJECT_SCOPE_PROFILE or not profile.get("rls_enabled") or not profile.get("rls_forced"):
                    fail(errors, f"{family_id}: project family lacks forced-RLS Project Scope profile")
                if profile.get("composite_reference_rule") != "MATCH FULL(owner_user_id, project_id, target_id)":
                    fail(errors, f"{family_id}: project family lacks MATCH FULL composite-reference rule")
                if "storyos_runtime" not in profile.get("runtime_grants", []):
                    fail(errors, f"{family_id}: project family lacks runtime grant profile")
        schema = family.get("schema", {})
        if schema.get("current") != ACTIVE_SCHEMA:
            fail(errors, f"{family_id}: current schema is not active Release 1")
        if not isinstance(schema.get("migration_class"), str) or not schema["migration_class"]:
            fail(errors, f"{family_id}: schema migration_class is required")
        supported = schema.get("supported_predecessors", [])
        if (
            not isinstance(supported, list)
            or any(not isinstance(item, str) for item in supported)
            or len(supported) != len(set(supported))
        ):
            fail(errors, f"{family_id}: supported predecessors must be a unique string list")
        elif supported != global_predecessors:
            fail(errors, f"{family_id}: supported predecessors must exactly equal the global migration predecessor set")
        for dependency in family.get("dependencies", []):
            if dependency not in family_ids:
                fail(errors, f"{family_id}: unknown dependency {dependency}")
        for field in ("transaction_group", "immutability", "head", "projection", "portability", "public_contract"):
            if field not in family:
                fail(errors, f"{family_id}: missing required catalog field {field}")
        projection = family.get("projection", {})
        projection_kind = projection.get("kind")
        if projection_kind not in ALLOWED_PROJECTION_KINDS:
            fail(errors, f"{family_id}: invalid projection kind {projection_kind!r}")
        if projection_kind == "none":
            if projection.get("source_family_ids") or projection.get("dependency_closure"):
                fail(errors, f"{family_id}: non-projection family declares projection dependencies")
        else:
            required_projection_fields = ("source_family_ids", "dependency_closure", "generation_field", "watermark_field", "invalidation_families", "rebuild", "visibility_gate")
            if any(not projection.get(field) for field in required_projection_fields):
                fail(errors, f"{family_id}: projection lacks source/dependency/generation/watermark/rebuild/visibility fields")
            source_family_ids = projection.get("source_family_ids", [])
            invalidation_families = projection.get("invalidation_families", [])
            if not isinstance(source_family_ids, list) or any(not isinstance(item, str) for item in source_family_ids):
                fail(errors, f"{family_id}: projection source families must be strings")
            elif len(source_family_ids) != len(set(source_family_ids)):
                fail(errors, f"{family_id}: projection source families must be unique")
            if not isinstance(invalidation_families, list) or any(not isinstance(item, str) for item in invalidation_families):
                fail(errors, f"{family_id}: projection invalidation families must be strings")
            elif len(invalidation_families) != len(set(invalidation_families)):
                fail(errors, f"{family_id}: projection invalidation families must be unique")
            if isinstance(source_family_ids, list) and all(isinstance(item, str) for item in source_family_ids):
                for source_family_id in source_family_ids:
                    if source_family_id not in family_ids:
                        fail(errors, f"{family_id}: projection references unknown source family {source_family_id}")
            if isinstance(invalidation_families, list) and all(isinstance(item, str) for item in invalidation_families):
                for invalidation_family_id in invalidation_families:
                    if invalidation_family_id not in family_ids:
                        fail(errors, f"{family_id}: projection references unknown invalidation family {invalidation_family_id}")
            if projection_kind == "disposable" and ("project-canonical" not in source_family_ids or "project-canonical" not in family.get("dependencies", [])):
                fail(errors, f"{family_id}: disposable projection lacks a canonical Project dependency")
            if family.get("authority_class") != "projection" and projection_kind == "disposable":
                fail(errors, f"{family_id}: disposable projection must use projection authority class")
        portability = family.get("portability", {})
        required_portability_fields = {"physical_backup", "whole_service_restore", "project_export", "project_restore", "secret_exclusion"}
        if not required_portability_fields <= set(portability):
            fail(errors, f"{family_id}: portability classification is incomplete")
        for field, allowed_values in ALLOWED_PORTABILITY.items():
            value = portability.get(field)
            if value not in allowed_values:
                fail(errors, f"{family_id}: portability.{field} has closed-set violation {value!r}")
        if portability.get("secret_exclusion") not in {"no_secret_material", "reference_only"}:
            fail(errors, f"{family_id}: secret exclusion classification is not fail-closed")
        if family.get("authority_class") == "projection" and projection_kind == "none":
            fail(errors, f"{family_id}: projection authority class requires a projection kind")
        if family.get("authority_class") == "projection":
            if portability.get("project_export") not in {"exclude", "not_applicable"}:
                fail(errors, f"{family_id}: disposable projection cannot enter Project Export")
            if portability.get("project_restore") != "rebuild":
                fail(errors, f"{family_id}: projection restore must be rebuild")
            if portability.get("physical_backup") != "metadata_only":
                fail(errors, f"{family_id}: projection backup must be metadata-only")
            if portability.get("whole_service_restore") not in {
                "rebuild",
                "rebuild_or_restore_and_verify",
                "rebuild_or_unavailable",
                "restore_metadata_then_rebuild",
            }:
                fail(errors, f"{family_id}: projection whole-service restore must rebuild or explicitly gate unavailability")
        if family.get("scope", {}).get("kind") == "project" and family.get("authority_class") in {"canonical", "artifact", "operational"}:
            if portability.get("physical_backup") == "metadata_only":
                fail(errors, f"{family_id}: durable project history cannot be metadata-only in physical backup")
            if portability.get("project_export") in {"exclude", "not_applicable"}:
                fail(errors, f"{family_id}: durable project history cannot be excluded from Project Export")
            if portability.get("whole_service_restore") in {"rebuild", "rebuild_or_unavailable", "restore_metadata_then_rebuild"}:
                fail(errors, f"{family_id}: durable project history cannot be discarded and rebuilt as a projection")
        if portability.get("secret_exclusion") == "reference_only":
            if portability.get("physical_backup") != "full_rows_without_secret_values":
                fail(errors, f"{family_id}: reference-only data must exclude secret values from physical backup")
            if portability.get("project_export") not in {"include_nonsecret_evidence", "include_reference_only", "archive_manifest_and_entries", "not_applicable"}:
                fail(errors, f"{family_id}: reference-only data has an unsafe Project Export classification")
            if portability.get("project_restore") not in {"restore_and_unbound_credentials", "stage_validate_promote_atomically", "not_applicable"}:
                fail(errors, f"{family_id}: reference-only data has an unsafe Project Restore classification")
        if family_id == "credential-references" and portability != {
            "physical_backup": "full_rows_without_secret_values",
            "whole_service_restore": "restore_and_mark_unbound",
            "project_export": "include_reference_only",
            "project_restore": "restore_and_unbound_credentials",
            "secret_exclusion": "reference_only",
        }:
            fail(errors, f"{family_id}: Credential Reference portability must be reference-only and unbound on restore")
        if family.get("scope", {}).get("kind") == "global" and portability.get("project_export") == "include":
            fail(errors, f"{family_id}: global definition cannot be copied into a Project Export")
        public = family.get("public_contract", {})
        if public.get("mapping_role") is None:
            fail(errors, f"{family_id}: public_contract.mapping_role is required")
        explicit_operation_ids = public.get("operation_ids", [])
        if not isinstance(explicit_operation_ids, list) or any(not isinstance(item, str) for item in explicit_operation_ids):
            fail(errors, f"{family_id}: public_contract.operation_ids must be a string list")
            explicit_operation_ids = []
        if len(explicit_operation_ids) != len(set(explicit_operation_ids)):
            fail(errors, f"{family_id}: public_contract.operation_ids must be unique")
        selected_operations = set(explicit_operation_ids)
        unknown_explicit = selected_operations - set(operation_by_id)
        if unknown_explicit:
            fail(errors, f"{family_id}: unknown operation_ids {sorted(unknown_explicit)}")
        selected_operations |= selector_operations(public.get("operation_selector"), operations, errors, f"{family_id}.public_contract")
        selected_events = selector_events(public.get("event_selector"), events, errors, f"{family_id}.public_contract")
        if not selected_operations and not selected_events and public.get("internal_only") is not True:
            fail(errors, f"{family_id}: family without public mapping must be explicitly internal_only")
        settlement_modes = public.get("settlement_modes")
        if not isinstance(settlement_modes, list) or any(not isinstance(mode, str) for mode in settlement_modes):
            fail(errors, f"{family_id}: public_contract.settlement_modes must be a string list")
            settlement_modes = []
        if len(settlement_modes) != len(set(settlement_modes)):
            fail(errors, f"{family_id}: public_contract.settlement_modes must be unique")
        unknown_modes = set(settlement_modes) - ALLOWED_SETTLEMENT_MODES
        if unknown_modes:
            fail(errors, f"{family_id}: public_contract.settlement_modes has closed-set violation {sorted(unknown_modes)}")
        actual_modes = {
            operation_by_id[operation_id].get("settlement", {}).get("mode")
            for operation_id in selected_operations
            if operation_id in operation_by_id
        }
        if actual_modes != set(settlement_modes):
            fail(errors, f"{family_id}: settlement_modes {sorted(settlement_modes)} do not exactly cover route settlement modes {sorted(actual_modes)}")
        for operation_id in selected_operations:
            operation_mode = operation_by_id.get(operation_id, {}).get("settlement", {}).get("mode")
            if operation_mode not in set(settlement_modes):
                fail(errors, f"{family_id}: operation {operation_id} has uncovered route settlement mode {operation_mode!r}")
        wire_history = public.get("wire_history")
        if wire_history not in ALLOWED_WIRE_HISTORY:
            fail(errors, f"{family_id}: public_contract.wire_history has closed-set violation {wire_history!r}")
        declared_activity_profile = public.get("activity_profile")
        referenced_event_ids: set[str] = set(selected_events)
        for operation_id in selected_operations:
            operation = operation_by_id.get(operation_id)
            if not operation:
                continue
            activity = operation.get("activity", {})
            referenced_event_ids.update(activity.get("success", []))
            referenced_event_ids.update(activity.get("no_effect", []))
            route_wire_owner = operation.get("wire_owner")
            if wire_history == "canonical" and route_wire_owner != family.get("owner"):
                fail(errors, f"{family_id}: canonical wire-history operation {operation_id} is owned by route wire owner {route_wire_owner!r}, not catalog owner {family.get('owner')!r}")
            if wire_history in {"referenced_by_wire_history", "event_representation_referenced"} and route_wire_owner != "protocol":
                fail(errors, f"{family_id}: referenced wire-history operation {operation_id} does not name the protocol route wire owner")
        known_referenced_events = {event_id for event_id in referenced_event_ids if event_id in event_by_id}
        actual_activity_profiles = {event_by_id[event_id].get("wire_profile") for event_id in known_referenced_events}
        if actual_activity_profiles and actual_activity_profiles != {declared_activity_profile}:
            fail(errors, f"{family_id}: activity_profile {declared_activity_profile!r} disagrees with route Event wire profiles {sorted(actual_activity_profiles)}")
        if selected_events and declared_activity_profile is None:
            fail(errors, f"{family_id}: Event selector requires an activity_profile")
        if public.get("mapping_role") in {"activity_owner", "wire_history"} and not selected_events:
            fail(errors, f"{family_id}: {public.get('mapping_role')} must select the route Event set")
        for operation_id in selected_operations:
            operation_owners[operation_id].add(family_id)
        for event_id in selected_events:
            event_owners[event_id].add(family_id)
    dependency_graph = {family_id: list(family.get("dependencies", [])) for family_id, family in family_map.items()}
    if graph_has_cycle(dependency_graph):
        fail(errors, "family dependency graph contains a cycle")
    return family_map, operation_owners, event_owners


def validate_physical_ledger_boundary(catalog: dict[str, Any], family_map: dict[str, dict[str, Any]], errors: list[str]) -> None:
    ledger = catalog.get("physical_ledger", {})
    if ledger.get("authority") != "sole_exhaustive_table_family_ledger":
        fail(errors, "physical_ledger must declare the sole exhaustive table-family authority")
    if ledger.get("source") != "docs/foundation/postgresql-release-1-persistence-catalog.json":
        fail(errors, "physical_ledger.source must be the persistence catalog itself")
    boundary = ledger.get("contract_boundary")
    expected_boundary = "docs/foundation/postgresql-project-storage-isolation-and-migration-contract.md#32-normative-aggregate-and-family-boundary"
    if boundary != expected_boundary:
        fail(errors, "physical_ledger.contract_boundary must bind the normative aggregate/family section")
    validate_source("physical_ledger.contract_boundary", boundary, errors)

    contract_text = STORAGE_CONTRACT_PATH.read_text(encoding="utf-8")
    start_marker = "### 3.2 Normative aggregate and family boundary"
    end_marker = "### 3.3 Release 1 family responsibility map"
    start = contract_text.find(start_marker)
    end = contract_text.find(end_marker)
    if start == -1 or end <= start:
        fail(errors, "contract does not contain a bounded 3.2/3.3 family-ledger section")
        return
    boundary_text = contract_text[start:end]
    if "sole exhaustive" not in boundary_text or "physical table-family ledger" not in boundary_text:
        fail(errors, "contract 3.2 does not declare the catalog as the sole exhaustive physical ledger")
    backtick_tokens = set(re.findall(r"`([^`]+)`", boundary_text))
    family_ids = set(family_map)
    if (backtick_tokens & family_ids) != family_ids:
        fail(errors, "contract 3.2 family IDs do not exactly cover the catalog family IDs")
    table_names = {
        table_name
        for family in family_map.values()
        for table_name in family.get("table_families", [])
        if isinstance(table_name, str)
    }
    if backtick_tokens & table_names:
        fail(errors, f"contract 3.2 repeats physical table names outside the sole catalog ledger: {sorted(backtick_tokens & table_names)}")

    registry = catalog.get("table_family_registry", {})
    if registry.get("profile") != "storyos.persistence.table-family-registry.v1":
        fail(errors, "table_family_registry.profile is missing or incorrect")
    if registry.get("digest_algorithm") != "sha256:canonical-json":
        fail(errors, "table_family_registry.digest_algorithm must be sha256:canonical-json")
    expected_covers = ["family_id", "table_families", "owner", "authority_class", "durability_class"]
    if registry.get("covers") != expected_covers:
        fail(errors, "table_family_registry.covers must bind table name, family, owner, authority, and durability")
    registry_entries = [
        {
            "family_id": family_id,
            "table_families": family.get("table_families"),
            "owner": family.get("owner"),
            "authority_class": family.get("authority_class"),
            "durability_class": family.get("durability_class"),
        }
        for family_id, family in family_map.items()
    ]
    expected_digest = "sha256:" + hashlib.sha256(canonical_bytes(registry_entries)).hexdigest()
    if registry.get("digest") != expected_digest:
        fail(errors, f"table-family registry digest mismatch: expected {expected_digest}")
