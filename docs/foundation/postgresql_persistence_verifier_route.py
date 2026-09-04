"""Release 1 route, settlement, Activity, and Wire coverage validation."""

from __future__ import annotations

from typing import Any

from postgresql_persistence_verifier_common import (
    ALLOWED_SETTLEMENT_MODES,
    fail,
    selector_events,
    selector_operations,
)


def validate_route_coverage(catalog: dict[str, Any], route_catalog: dict[str, Any], family_map: dict[str, dict[str, Any]], operation_owners: dict[str, set[str]], event_owners: dict[str, set[str]], errors: list[str]) -> dict[str, int]:
    coverage = catalog.get("route_coverage", {})
    operations = route_catalog.get("operations", [])
    events = route_catalog.get("events", [])
    operation_by_id = {operation.get("operation_id"): operation for operation in operations}
    event_by_id = {event.get("schema_id"): event for event in events}
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
            if settlement.get("mode") not in ALLOWED_SETTLEMENT_MODES:
                fail(errors, f"{operation_id}: settlement mode is outside the closed Release 1 set")
            if not isinstance(settlement.get("acknowledgements"), list) or any(not isinstance(item, str) for item in settlement.get("acknowledgements", [])):
                fail(errors, f"{operation_id}: settlement acknowledgements must be a string list")
            if "accepted" in settlement.get("acknowledgements", []):
                query_id = settlement.get("settlement_query_operation_id")
                query = operation_by_id.get(query_id)
                if not isinstance(query, dict) or query.get("kind") != "query" or query.get("method") != "GET":
                    fail(errors, f"{operation_id}: Accepted settlement lacks an exact GET settlement query")
                elif query.get("path") != settlement.get("settlement_query"):
                    fail(errors, f"{operation_id}: settlement query path mismatch")
                query_owners = operation_owners.get(query_id, set()) if isinstance(query_id, str) else set()
                durable_query_owners = {
                    owner_id
                    for owner_id in query_owners
                    if family_map.get(owner_id, {}).get("authority_class") != "projection"
                    and family_map.get(owner_id, {}).get("public_contract", {}).get("internal_only") is not True
                }
                if not durable_query_owners:
                    fail(errors, f"{operation_id}: Accepted settlement query has no durable non-projection owner")
            elif settlement.get("settlement_query_operation_id") is not None or settlement.get("settlement_query") is not None:
                fail(errors, f"{operation_id}: non-accepted settlement must not declare a settlement query")
            activity = operation.get("activity", {})
            activity_event_ids = [*activity.get("success", []), *activity.get("no_effect", [])]
            for event_id in activity_event_ids:
                if event_id not in event_by_id:
                    fail(errors, f"{operation_id}: Activity mapping names unknown Event {event_id}")
                elif coverage.get("activity_owner_family_id") not in event_owners.get(event_id, set()):
                    fail(errors, f"{operation_id}: Event {event_id} is not physically owned by the activity family")
                elif event_by_id[event_id].get("wire_profile") != "storyos.project-activity.v1":
                    fail(errors, f"{operation_id}: Activity Event {event_id} does not use the Release 1 Project Activity wire profile")
    required_events = selector_events(coverage.get("required_events"), events, errors, "route_coverage.required_events")
    activity_family = coverage.get("activity_owner_family_id")
    activity_mapping = family_map.get(activity_family, {}).get("public_contract", {}).get("mapping_role")
    if activity_mapping != "activity_owner":
        fail(errors, "route coverage activity owner is not an activity_owner family")
    for event_id in required_events:
        if activity_family not in event_owners.get(event_id, set()):
            fail(errors, f"Release 1 Event lacks activity physical coverage: {event_id}")
        elif event_by_id.get(event_id, {}).get("wire_profile") != family_map.get(activity_family, {}).get("public_contract", {}).get("activity_profile"):
            fail(errors, f"Activity owner profile does not match route Event wire profile: {event_id}")
    wire_family = coverage.get("wire_history_family_id")
    if family_map.get(wire_family, {}).get("public_contract", {}).get("mapping_role") != "wire_history":
        fail(errors, "route coverage wire owner is not a wire_history family")
    if not any(wire_family in owners for owners in event_owners.values()):
        fail(errors, "wire-history family does not cover any public Event representation")
    if wire_family not in {owner for owners in operation_owners.values() for owner in owners}:
        fail(errors, "wire-history family does not cover persisted command identities")
    wire_owner = family_map.get(wire_family, {}).get("owner")
    for operation in operations:
        if operation.get("kind") in {"command", "challenge"} and operation.get("operation_id") in required:
            if operation.get("wire_owner") != wire_owner:
                fail(errors, f"{operation.get('operation_id')}: route wire owner does not match canonical wire-history family owner")
    projection_operations = {"searchManuscript", "retrieveContext"}
    for operation_id in projection_operations:
        if not any(family_map.get(family_id, {}).get("authority_class") == "projection" for family_id in operation_owners.get(operation_id, set())):
            fail(errors, f"projection-bearing operation lacks a projection family: {operation_id}")
    outcome_query_families = {
        "project-canonical",
        "operational-receipts-actions",
        "operational-admission-editor",
        "operational-project-activity",
    }
    if "getApplyAuthorEditOutcome" in operation_by_id and operation_owners.get(
        "getApplyAuthorEditOutcome", set()
    ) != outcome_query_families:
        fail(
            errors,
            "getApplyAuthorEditOutcome must map exactly to its canonical, Receipt, Admission, and applied-Activity read families",
        )
    expected_outcome_relation = {
        "schema_id": "storyos.persistence.apply-author-edit-outcome-read.v1",
        "operation_id": "getApplyAuthorEditOutcome",
        "family_ids": [
            "project-canonical",
            "operational-receipts-actions",
            "operational-admission-editor",
            "operational-project-activity",
        ],
        "committed_projection": "receipt_first_exact_replay",
        "outcomes": ["committed", "rejected", "requires_reconfirmation", "still_unknown"],
        "capabilities": ["read"],
        "nonce_consumption": "none",
        "core_invocation": "already_admitted_direct_editor_action",
        "lifecycle_append": "same_admission_receipt_or_reconfirmation",
        "outcome_unknown_table_access": "none",
    }
    if catalog.get("author_edit_outcome_query_read") != expected_outcome_relation:
        fail(errors, "getApplyAuthorEditOutcome named outcome relation must equal the closed v1 shape")
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
