#!/usr/bin/env python3
"""Review-time proof for the Release 1 PostgreSQL persistence catalog.

This verifier validates the contract/catalog boundary only. It does not connect
to PostgreSQL, execute DDL, run a backup, or claim executable recovery proof.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[2]
CATALOG_PATH = ROOT / "docs/foundation/postgresql-release-1-persistence-catalog.json"
ROUTE_CATALOG_PATH = ROOT / "docs/foundation/versioned-protocol-release-1-route-catalog.json"
STORAGE_CONTRACT_PATH = ROOT / "docs/foundation/postgresql-project-storage-isolation-and-migration-contract.md"
ACTIVE_SCHEMA = "storyos.persistence.release-1.v1"
PROJECT_SCOPE_PROFILE = "project_forced_rls"
ALLOWED_AUTHORITY_CLASSES = {"identity", "canonical", "artifact", "operational", "projection", "administrative"}
ALLOWED_SCOPE_KINDS = {"user", "project", "global", "maintenance"}
ALLOWED_PROJECTION_KINDS = {"none", "disposable", "historical_projection", "projection_control"}


def github_heading_slug(heading: str) -> str:
    heading = re.sub(r"\s+#+\s*$", "", heading).strip()
    heading = re.sub(r"`([^`]*)`", r"\1", heading)
    heading = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", heading)
    heading = re.sub(r"<[^>]+>", "", heading)
    heading = re.sub(r"[^\w\s-]", "", heading.lower(), flags=re.UNICODE)
    return re.sub(r"[-\s]+", "-", heading).strip("-")


def source_anchors(path: Path) -> set[str]:
    return {
        github_heading_slug(match.group(1))
        for line in path.read_text(encoding="utf-8").splitlines()
        if (match := re.match(r"^#{1,6}\s+(.+?)\s*$", line))
    }


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def normalized_file_bytes(path: Path) -> bytes:
    return path.read_bytes().replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def validate_source(label: str, source: Any, errors: list[str]) -> None:
    if not isinstance(source, str):
        fail(errors, f"{label}: source must be a string")
        return
    source_path, _, anchor = source.partition("#")
    path = ROOT / source_path
    if not path.is_file():
        fail(errors, f"{label}: missing source {source}")
        return
    if ".reference" in path.parts:
        fail(errors, f"{label}: source enters .reference: {source}")
    if anchor and anchor not in source_anchors(path):
        fail(errors, f"{label}: missing Markdown anchor {source}")


def validate_relative_links(path: Path, errors: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    for index, line in enumerate(text.splitlines(), 1):
        for match in re.finditer(r"\[[^\]]+\]\(([^)]+)\)", line):
            target = match.group(1).strip().split()[0]
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target_path, _, anchor = target.partition("#")
            resolved = (path.parent / unquote(target_path)).resolve()
            if not resolved.is_file():
                fail(errors, f"{path}:{index}: missing relative link {target}")
            elif anchor and anchor not in source_anchors(resolved):
                fail(errors, f"{path}:{index}: missing relative-link anchor {target}")


def selector_operations(selector: Any, operations: list[dict[str, Any]], errors: list[str], label: str) -> set[str]:
    if selector is None:
        return set()
    if not isinstance(selector, dict) or not isinstance(selector.get("mode"), str):
        fail(errors, f"{label}: operation_selector must be an object with mode")
        return set()
    mode = selector["mode"]
    operation_ids = {operation.get("operation_id") for operation in operations}
    if mode == "explicit":
        selected = selector.get("operation_ids")
        if not isinstance(selected, list) or any(not isinstance(item, str) for item in selected):
            fail(errors, f"{label}: explicit selector requires string operation_ids")
            return set()
        unknown = set(selected) - operation_ids
        if unknown:
            fail(errors, f"{label}: selector references unknown operations {sorted(unknown)}")
        return set(selected) & operation_ids
    if mode == "all_except":
        excluded = selector.get("exclude", [])
        if not isinstance(excluded, list) or any(not isinstance(item, str) for item in excluded):
            fail(errors, f"{label}: all_except selector requires string exclude")
            return set()
        unknown = set(excluded) - operation_ids
        if unknown:
            fail(errors, f"{label}: all_except references unknown operations {sorted(unknown)}")
        return operation_ids - set(excluded)
    if mode == "all_persisted_commands":
        return {
            operation["operation_id"]
            for operation in operations
            if operation.get("kind") in {"command", "challenge"}
        }
    if mode == "all_author_command_operations":
        return {
            operation["operation_id"]
            for operation in operations
            if operation.get("kind") in {"command", "challenge"}
            and operation.get("admission") != "query"
        }
    fail(errors, f"{label}: unknown operation selector mode {mode!r}")
    return set()


def selector_events(selector: Any, events: list[dict[str, Any]], errors: list[str], label: str) -> set[str]:
    if selector is None:
        return set()
    if not isinstance(selector, dict) or selector.get("mode") != "all":
        fail(errors, f"{label}: event_selector must use the explicit all mode")
        return set()
    return {event.get("schema_id") for event in events}


def dependency_path_count(graph: dict[str, list[str]], start: str, target: str) -> int:
    memo: dict[str, int] = {}

    def visit(node: str, active: set[str]) -> int:
        if node == target:
            return 1
        if node in active:
            return 0
        if node in memo:
            return memo[node]
        total = sum(visit(child, active | {node}) for child in graph.get(node, []))
        memo[node] = total
        return total

    return visit(start, set())


def graph_has_cycle(graph: dict[str, list[str]]) -> bool:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> bool:
        if node in visiting:
            return True
        if node in visited:
            return False
        visiting.add(node)
        if any(visit(child) for child in graph.get(node, [])):
            return True
        visiting.remove(node)
        visited.add(node)
        return False

    return any(visit(node) for node in graph)


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
    if not isinstance(predecessors, list) or len(predecessors) != len(set(predecessors)):
        fail(errors, "migration_chain.supported_predecessors must be unique")
        predecessors = []
    if not isinstance(edges, list) or not edges:
        fail(errors, "migration_chain.edges must be non-empty")
        edges = []
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
    phases = catalog.get("migration_phases")
    if not isinstance(phases, list) or not phases:
        fail(errors, "migration_phases must be non-empty")
        phases = []
    phase_ids = [phase.get("phase_id") for phase in phases if isinstance(phase, dict)]
    if len(phase_ids) != len(set(phase_ids)):
        fail(errors, "migration phase IDs must be unique")
    phase_map = {phase.get("phase_id"): phase for phase in phases if isinstance(phase, dict)}
    required_phase_ids = set(edges[0].get("phase_ids", [])) if edges else set()
    if set(phase_map) != required_phase_ids:
        fail(errors, f"migration phase catalog does not equal edge phase set: {set(phase_map) ^ required_phase_ids}")
    for phase_id, phase in phase_map.items():
        for field in ("class", "transactional", "restartable", "idempotent", "postcondition", "failure_state"):
            if field not in phase:
                fail(errors, f"migration phase {phase_id} missing {field}")
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
    if dependency_path_count(transition_pairs, "predecessor_present", "active") == 0:
        fail(errors, "predecessor state has no positive path to active")
    active_gates = state_machine.get("active_write_gates", [])
    required_gates = {"same_release_identity_exact", "migration_chain_complete", "recovery_visibility_proof_current"}
    if not required_gates <= set(active_gates):
        fail(errors, "active write gates omit release, migration, or recovery visibility proof")


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
        if not isinstance(supported, list) or len(supported) != len(set(supported)):
            fail(errors, f"{family_id}: supported predecessors must be unique")
        elif not set(supported) <= set(catalog.get("migration_chain", {}).get("supported_predecessors", [])):
            fail(errors, f"{family_id}: unsupported predecessor appears in family schema")
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
        if portability.get("secret_exclusion") not in {"no_secret_material", "reference_only"}:
            fail(errors, f"{family_id}: secret exclusion classification is not fail-closed")
        if family.get("authority_class") == "projection":
            if portability.get("project_export") not in {"exclude", "not_applicable"}:
                fail(errors, f"{family_id}: disposable projection cannot enter Project Export")
            if portability.get("project_restore") != "rebuild":
                fail(errors, f"{family_id}: projection restore must be rebuild")
            if portability.get("physical_backup") not in {"metadata_only", "not_applicable"}:
                fail(errors, f"{family_id}: projection backup must be metadata-only")
        if family.get("scope", {}).get("kind") == "global" and portability.get("project_export") == "include":
            fail(errors, f"{family_id}: global definition cannot be copied into a Project Export")
        public = family.get("public_contract", {})
        if public.get("mapping_role") is None:
            fail(errors, f"{family_id}: public_contract.mapping_role is required")
        selected_operations = set(public.get("operation_ids", []))
        unknown_explicit = selected_operations - {operation.get("operation_id") for operation in operations}
        if unknown_explicit:
            fail(errors, f"{family_id}: unknown operation_ids {sorted(unknown_explicit)}")
        selected_operations |= selector_operations(public.get("operation_selector"), operations, errors, f"{family_id}.public_contract")
        selected_events = selector_events(public.get("event_selector"), events, errors, f"{family_id}.public_contract")
        if not selected_operations and not selected_events and public.get("internal_only") is not True:
            fail(errors, f"{family_id}: family without public mapping must be explicitly internal_only")
        for operation_id in selected_operations:
            operation_owners[operation_id].add(family_id)
        for event_id in selected_events:
            event_owners[event_id].add(family_id)
    dependency_graph = {family_id: list(family.get("dependencies", [])) for family_id, family in family_map.items()}
    if graph_has_cycle(dependency_graph):
        fail(errors, "family dependency graph contains a cycle")
    return family_map, operation_owners, event_owners


def validate_route_coverage(catalog: dict[str, Any], route_catalog: dict[str, Any], family_map: dict[str, dict[str, Any]], operation_owners: dict[str, set[str]], event_owners: dict[str, set[str]], errors: list[str]) -> dict[str, int]:
    coverage = catalog.get("route_coverage", {})
    operations = route_catalog.get("operations", [])
    events = route_catalog.get("events", [])
    if coverage.get("source") != "docs/foundation/versioned-protocol-release-1-route-catalog.json":
        fail(errors, "route_coverage.source must be the checked-in Release 1 route catalog")
    expected_settlement_fields = {
        "operation_id",
        "settlement.mode",
        "settlement.acknowledgements",
        "settlement.settlement_query_operation_id",
        "activity.success",
        "activity.no_effect",
    }
    settlement_fields = coverage.get("settlement_identity_fields", [])
    if not isinstance(settlement_fields, list) or any(not isinstance(item, str) for item in settlement_fields) or set(settlement_fields) != expected_settlement_fields or len(settlement_fields) != len(expected_settlement_fields):
        fail(errors, "route coverage settlement identity fields are incomplete or duplicated")
    required = selector_operations(coverage.get("required_operations"), operations, errors, "route_coverage.required_operations")
    excluded = set(coverage.get("excluded_operations", {}))
    if required & excluded:
        fail(errors, f"route coverage excludes required operations {sorted(required & excluded)}")
    for operation in operations:
        operation_id = operation.get("operation_id")
        if operation_id in required and not operation_owners.get(operation_id):
            fail(errors, f"Release 1 operation has no persistence family coverage: {operation_id}")
        settlement = operation.get("settlement", {})
        if operation_id in required:
            for field in ("mode", "acknowledgements", "http_statuses"):
                if field not in settlement:
                    fail(errors, f"{operation_id}: settlement identity is incomplete")
            if "accepted" in settlement.get("acknowledgements", []):
                query_id = settlement.get("settlement_query_operation_id")
                query = next((candidate for candidate in operations if candidate.get("operation_id") == query_id), None)
                if not isinstance(query, dict) or query.get("kind") != "query" or query.get("method") != "GET":
                    fail(errors, f"{operation_id}: Accepted settlement lacks an exact GET settlement query")
                elif query.get("path") != settlement.get("settlement_query"):
                    fail(errors, f"{operation_id}: settlement query path mismatch")
            activity = operation.get("activity", {})
            for event_id in [*activity.get("success", []), *activity.get("no_effect", [])]:
                if event_id not in {event.get("schema_id") for event in events}:
                    fail(errors, f"{operation_id}: Activity mapping names unknown Event {event_id}")
                elif coverage.get("activity_owner_family_id") not in event_owners.get(event_id, set()):
                    fail(errors, f"{operation_id}: Event {event_id} is not physically owned by the activity family")
    required_events = selector_events(coverage.get("required_events"), events, errors, "route_coverage.required_events")
    activity_family = coverage.get("activity_owner_family_id")
    activity_mapping = family_map.get(activity_family, {}).get("public_contract", {}).get("mapping_role")
    if activity_mapping != "activity_owner":
        fail(errors, "route coverage activity owner is not an activity_owner family")
    for event_id in required_events:
        if activity_family not in event_owners.get(event_id, set()):
            fail(errors, f"Release 1 Event lacks activity physical coverage: {event_id}")
    wire_family = coverage.get("wire_history_family_id")
    if family_map.get(wire_family, {}).get("public_contract", {}).get("mapping_role") != "wire_history":
        fail(errors, "route coverage wire owner is not a wire_history family")
    if not any(wire_family in owners for owners in event_owners.values()):
        fail(errors, "wire-history family does not cover any public Event representation")
    if wire_family not in {owner for owners in operation_owners.values() for owner in owners}:
        fail(errors, "wire-history family does not cover persisted command identities")
    projection_operations = {"searchManuscript", "retrieveContext"}
    for operation_id in projection_operations:
        if not any(family_map.get(family_id, {}).get("authority_class") == "projection" for family_id in operation_owners.get(operation_id, set())):
            fail(errors, f"projection-bearing operation lacks a projection family: {operation_id}")
    routed_families = set().union(*operation_owners.values(), *event_owners.values()) if operation_owners or event_owners else set()
    internal_without_route = sum(
        1
        for family_id, family in family_map.items()
        if family.get("public_contract", {}).get("internal_only") is True and family_id not in routed_families
    )
    return {
        "required_operation_count": len(required),
        "covered_operation_count": sum(1 for operation_id in required if operation_owners.get(operation_id)),
        "required_event_count": len(required_events),
        "covered_event_count": sum(1 for event_id in required_events if activity_family in event_owners.get(event_id, set())),
        "internal_family_count": internal_without_route,
    }


def validate_catalog(catalog: dict[str, Any], route_catalog: dict[str, Any], errors: list[str], check_digests: bool = True) -> dict[str, int]:
    if catalog.get("catalog_id") != catalog.get("schema_identity", {}).get("persisted_format_catalog_id"):
        fail(errors, "catalog identity does not bind persisted-format identity")
    if catalog.get("contract_revision") != "release1-storage-contract-2026-07-31":
        fail(errors, "unexpected storage contract revision")
    binding = catalog.get("schema_identity", {}).get("protocol_binding", {})
    for key, expected in {
        "route_catalog_id": route_catalog.get("catalog_id"),
        "route_catalog_contract_revision": route_catalog.get("contract_revision"),
        "public_release": route_catalog.get("public_release"),
        "compatibility_profile": route_catalog.get("compatibility_profile"),
    }.items():
        if binding.get(key) != expected:
            fail(errors, f"protocol binding {key} does not match #58 route catalog")
    release_identity = route_catalog.get("release_identity", {})
    if binding.get("release_identity_schema_id") != release_identity.get("schema_id"):
        fail(errors, "storage catalog is not bound to the #58 release identity schema")
    if set(binding.get("required_persistence_identity_fields", [])) != {
        "database_schema_identity", "active_schema_version", "persisted_format_catalog_id", "migration_chain_id", "migration_chain_digest"
    }:
        fail(errors, "storage identity binding fields are incomplete")
    if check_digests:
        route_digest = "sha256:" + hashlib.sha256(normalized_file_bytes(ROUTE_CATALOG_PATH)).hexdigest()
        if binding.get("route_catalog_sha256") != route_digest:
            fail(errors, f"route catalog digest mismatch: expected {route_digest}")
        migration_copy = copy.deepcopy(catalog.get("migration_chain", {}))
        migration_copy.pop("digest_excludes", None)
        migration_digest = "sha256:" + hashlib.sha256(canonical_bytes(migration_copy)).hexdigest()
        if catalog.get("schema_identity", {}).get("migration_chain_digest") != migration_digest:
            fail(errors, f"migration chain digest mismatch: expected {migration_digest}")
    for owner_name, owner_source in catalog.get("owners", {}).items():
        validate_source(f"owner {owner_name}", owner_source, errors)
    validate_source("route_coverage.source", catalog.get("route_coverage", {}).get("source"), errors)
    validate_relative_links(STORAGE_CONTRACT_PATH, errors)
    roles = catalog.get("role_profiles", {})
    expected_roles = {"storyos_owner", "storyos_runtime", "storyos_migrator", "storyos_backup", "storyos_restore"}
    if set(roles) != expected_roles:
        fail(errors, "role_profiles must exactly cover owner/runtime/migrator/backup/restore")
    for role_name, role in roles.items():
        if role.get("superuser") is not False or role.get("bypass_rls") is not False:
            fail(errors, f"{role_name}: superuser and BYPASSRLS must both be false")
    if roles.get("storyos_owner", {}).get("login") is not False:
        fail(errors, "storyos_owner must be NOLOGIN")
    if roles.get("storyos_runtime", {}).get("request_pool") is not True:
        fail(errors, "storyos_runtime must be the only request-pool role")
    for role_name in expected_roles - {"storyos_runtime"}:
        if roles.get(role_name, {}).get("request_pool") is True:
            fail(errors, f"{role_name} must be absent from the request pool")
    validate_migration(catalog, errors)
    family_map, operation_owners, event_owners = validate_families(catalog, route_catalog, errors)
    route_stats = validate_route_coverage(catalog, route_catalog, family_map, operation_owners, event_owners, errors)
    if catalog.get("verification_contract", {}).get("require_negative_self_test") is not True:
        fail(errors, "catalog must require a negative self-test")
    return {
        "family_count": len(family_map),
        "table_family_count": sum(len(family.get("table_families", [])) for family in family_map.values()),
        "project_scoped_family_count": sum(1 for family in family_map.values() if family.get("scope", {}).get("requires_project_scope") is True),
        "projection_family_count": sum(1 for family in family_map.values() if family.get("authority_class") == "projection"),
        **route_stats,
    }


def main() -> int:
    raw = CATALOG_PATH.read_bytes()
    if b"\r" in raw:
        print("FAIL: catalog contains CR bytes", file=sys.stderr)
        return 1
    if not raw.endswith(b"\n"):
        print("FAIL: catalog is missing a final LF", file=sys.stderr)
        return 1
    try:
        catalog = json.loads(raw.decode("utf-8"))
        route_catalog = json.loads(normalized_file_bytes(ROUTE_CATALOG_PATH).decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        print(f"FAIL: invalid UTF-8/JSON: {exc}", file=sys.stderr)
        return 1
    errors: list[str] = []
    stats = validate_catalog(catalog, route_catalog, errors)
    if "--self-test" in sys.argv:
        probe = copy.deepcopy(catalog)
        probe["families"][-1]["family_id"] = probe["families"][0]["family_id"]
        probe_errors: list[str] = []
        validate_catalog(probe, route_catalog, probe_errors, check_digests=False)
        if not any("duplicate family_id" in error for error in probe_errors):
            errors.append("negative self-test did not reject duplicate family identity")
        anchor_probe = copy.deepcopy(catalog)
        owner_source, _, _ = anchor_probe["owners"]["storage"].partition("#")
        anchor_probe["owners"]["storage"] = f"{owner_source}#catalog-verifier-negative-anchor"
        anchor_errors: list[str] = []
        validate_catalog(anchor_probe, route_catalog, anchor_errors, check_digests=False)
        if not any("missing Markdown anchor" in error for error in anchor_errors):
            errors.append("negative self-test did not reject a missing owner anchor")
    if errors:
        print("FAIL:")
        for error in errors:
            print(f"- {error}")
        return 1
    print(
        "OK: "
        f"{stats['family_count']} families, {stats['table_family_count']} table families, "
        f"{stats['project_scoped_family_count']} Project-scoped families, "
        f"{stats['projection_family_count']} projection families; "
        f"{stats['covered_operation_count']}/{stats['required_operation_count']} required operations and "
        f"{stats['covered_event_count']}/{stats['required_event_count']} public Events covered"
    )
    if "--self-test" in sys.argv:
        print("OK: duplicate-family negative self-test rejected a bad catalog")
        print("OK: missing-owner-anchor negative self-test rejected a bad catalog")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
