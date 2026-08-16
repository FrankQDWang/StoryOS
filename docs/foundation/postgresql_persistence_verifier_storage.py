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
    STORAGE_CONTRACT_PATH,
    canonical_bytes,
    dependency_path_count,
    fail,
    graph_has_cycle,
    selector_events,
    selector_operations,
    validate_source,
)


def validate_migration(catalog: dict[str, Any], errors: list[str]) -> None:
    identity = catalog.get("schema_identity", {})
    migration = catalog.get("migration_chain", {})
    if identity.get("active_schema_version") != ACTIVE_SCHEMA:
        fail(errors, "schema_identity.active_schema_version must be the Release 1 schema")
    if identity.get("persisted_format_catalog_id") != catalog.get("catalog_id"):
        fail(errors, "persisted_format_catalog_id must equal catalog_id")
    if identity.get("migration_chain_id") != migration.get("chain_id"):
        fail(errors, "schema identity and migration chain identities disagree")
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
