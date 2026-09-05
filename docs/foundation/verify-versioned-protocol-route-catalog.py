#!/usr/bin/env python3
"""Review-time conformance checks for the pre-implementation Release 1 route catalog."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
CATALOG_PATH = ROOT / "docs/foundation/versioned-protocol-release-1-route-catalog.json"
PROTOCOL_PATH = ROOT / "docs/foundation/versioned-command-query-artifact-event-protocol.md"
ALLOWED_KINDS = {"command", "query", "challenge", "stream"}
ALLOWED_METHODS = {"GET", "POST", "PATCH", "PUT", "DELETE"}
EDITOR_ACTIONS = {"direct_editor_action", "explicit_editor_command"}
CONDITIONAL_CAUSE_ACTION = "conditional_by_cause"
REQUIRED_RELEASE1_CAPABILITY_IDS = {
    "project_lifecycle",
    "volume_chapter_lifecycle",
    "chapter_navigation",
    "manuscript_search",
    "direct_author_edit",
    "single_match_replace",
    "multi_match_cross_location_proposal",
    "manuscript_statistics",
    "human_readable_manuscript_export",
    "portable_project_archive",
    "editor_session_takeover",
    "snapshot_resync",
    "proposal_draft_lifecycle",
    "project_activity",
    "acknowledgement_loss_outcome_query",
}


def fail(message: str, errors: list[str]) -> None:
    errors.append(message)


def event_schema_family(schema_id: str, event_kind: str) -> str | None:
    dashed = event_kind.replace("_", "-")
    prefix = f"storyos.event.{dashed}.v"
    suffix = schema_id[len(prefix):] if schema_id.startswith(prefix) else ""
    if suffix.isdigit():
        return prefix
    return None


def source_path(source: str) -> Path:
    return ROOT / source.split("#", 1)[0]


def github_heading_slug(heading: str) -> str:
    """Return the stable GitHub-style slug used by tracked Markdown anchors."""
    heading = re.sub(r"\s+#+\s*$", "", heading).strip()
    heading = re.sub(r"`([^`]*)`", r"\1", heading)
    heading = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", heading)
    heading = re.sub(r"<[^>]+>", "", heading)
    heading = re.sub(r"[^\w\s-]", "", heading.lower(), flags=re.UNICODE)
    return re.sub(r"[-\s]+", "-", heading).strip("-")


def source_anchors(path: Path) -> set[str]:
    anchors: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^#{1,6}\s+(.+?)\s*$", line)
        if match:
            anchors.add(github_heading_slug(match.group(1)))
    return anchors


def validate_source_reference(
    label: str, source: Any, errors: list[str]
) -> None:
    if not isinstance(source, str):
        fail(f"{label} must be a tracked source string: {source!r}", errors)
        return
    path = source_path(source)
    if not path.is_file():
        fail(f"{label} points to a missing tracked source: {source!r}", errors)
        return
    if ".reference" in source:
        fail(f"{label} points into .reference: {source!r}", errors)
    if "#" in source:
        anchor = source.split("#", 1)[1]
        if anchor not in source_anchors(path):
            fail(f"{label} points to a missing Markdown anchor: {source!r}", errors)


def validate_conditional_cause_profiles(
    operation: dict[str, Any],
    owners: dict[str, Any],
    error_profiles: dict[str, Any],
    stable_error_rows: dict[str, int],
    errors: list[str],
) -> set[str]:
    operation_id = operation.get("operation_id", "<missing>")
    profiles = operation.get("cause_profiles")
    if operation.get("action_class") != CONDITIONAL_CAUSE_ACTION:
        if profiles is not None:
            fail(f"{operation_id}: cause_profiles require conditional_by_cause action", errors)
        return set()

    discriminator = operation.get("cause_discriminator")
    if not isinstance(discriminator, dict):
        fail(f"{operation_id}: conditional cause discriminator must be the cause field", errors)
    else:
        if discriminator.get("field") != "cause":
            fail(f"{operation_id}: conditional cause discriminator must be the cause field", errors)
        if discriminator.get("values") != ["author", "current_producer"]:
            fail(f"{operation_id}: conditional cause values must be author/current_producer", errors)
        if discriminator.get("semantic_owner") != operation.get("semantic_owner"):
            fail(f"{operation_id}: cause discriminator must use the operation semantic owner", errors)

    if not isinstance(profiles, dict) or set(profiles) != {"author", "current_producer"}:
        fail(f"{operation_id}: conditional cause profiles must be exactly author/current_producer", errors)
        return set()

    profile_schema_ids: set[str] = set()
    top_settlement = operation.get("settlement", {})
    top_statuses = set(top_settlement.get("http_statuses", [])) if isinstance(top_settlement, dict) else set()
    for cause_name, profile in profiles.items():
        prefix = f"{operation_id}.{cause_name}"
        required = {
            "semantic_cause",
            "cause_binding",
            "request_schema",
            "response_schema",
            "request_schema_source",
            "response_schema_source",
            "action_class",
            "admission",
            "editor",
            "idempotency",
            "settlement",
            "error_profile",
            "recovery",
        }
        if not isinstance(profile, dict):
            fail(f"{prefix}: cause profile must be an object", errors)
            continue
        missing = required - profile.keys()
        if missing:
            fail(f"{prefix}: missing {sorted(missing)}", errors)
            continue
        request_schema = profile.get("request_schema")
        response_schema = profile.get("response_schema")
        if not isinstance(request_schema, str) or not isinstance(response_schema, str):
            fail(f"{prefix}: request/response schema IDs are required", errors)
        else:
            if request_schema == response_schema:
                fail(f"{prefix}: request and response schema IDs must be distinct", errors)
            profile_schema_ids.update((request_schema, response_schema))
        if profile.get("request_schema_source") not in owners or profile.get("response_schema_source") not in owners:
            fail(f"{prefix}: schema source mapping is incomplete", errors)

        editor = profile.get("editor")
        if not isinstance(editor, dict) or not {"session", "writer_generation", "takeover"} <= editor.keys():
            fail(f"{prefix}: editor session/writer-generation/takeover rule is incomplete", errors)
        settlement = profile.get("settlement")
        profile_settlement = settlement if isinstance(settlement, dict) else {}
        if not isinstance(settlement, dict):
            fail(f"{prefix}: settlement profile must be an object", errors)
        else:
            statuses = settlement.get("http_statuses")
            if not isinstance(statuses, list) or not statuses or any(not isinstance(status, int) for status in statuses):
                fail(f"{prefix}: settlement HTTP statuses are incomplete", errors)
            elif set(statuses) != top_statuses:
                fail(f"{prefix}: cause settlement statuses must equal the conditional operation statuses", errors)
            if settlement.get("mode") != "sync":
                fail(f"{prefix}: WithdrawProposal/ReplanProposal cause settlement must be synchronous", errors)
            if settlement.get("settlement_query") is not None or settlement.get("operation_ref") is not None:
                fail(f"{prefix}: synchronous cause settlement cannot expose an async query reference", errors)
            acknowledgements = settlement.get("acknowledgements", [])
            if "accepted" in acknowledgements:
                fail(f"{prefix}: cause profile cannot acknowledge Accepted", errors)

        error_profile = profile.get("error_profile")
        if error_profile not in error_profiles:
            fail(f"{prefix}: unknown error profile {error_profile!r}", errors)
        else:
            missing_statuses = {
                stable_error_rows[code]
                for code in error_profiles[error_profile]
                if code in stable_error_rows
            } - set(profile_settlement.get("http_statuses", []))
            if missing_statuses:
                fail(f"{prefix}: error-profile HTTP statuses are missing {sorted(missing_statuses)}", errors)

        recovery = profile.get("recovery")
        profile_recovery = recovery if isinstance(recovery, dict) else {}
        if not isinstance(recovery, dict) or recovery.get("requires_reconfirmation") not in {"allowed", "forbidden"}:
            fail(f"{prefix}: recovery RequiresReconfirmation rule is incomplete", errors)

        if cause_name == "author":
            if profile.get("cause_binding") != "protected_author_command_admission":
                fail(f"{prefix}: author cause binding must be protected_author_command_admission", errors)
            if profile.get("action_class") != "explicit_editor_command":
                fail(f"{prefix}: author cause must be an explicit_editor_command", errors)
            if profile.get("admission") != "author_command":
                fail(f"{prefix}: author cause must use AuthorCommandAdmission", errors)
            if editor != {"session": "required", "writer_generation": "current", "takeover": "no"}:
                fail(f"{prefix}: author cause editor binding is incomplete", errors)
            if profile.get("idempotency") != "header_and_admission_record":
                fail(f"{prefix}: author cause idempotency must bind the admission record", errors)
            if profile_recovery.get("requires_reconfirmation") != "allowed":
                fail(f"{prefix}: author cause must allow RequiresReconfirmation", errors)
            if set(profile_settlement.get("acknowledgements", [])) != {"committed", "requires_reconfirmation"}:
                fail(f"{prefix}: author cause acknowledgements are incomplete", errors)
        elif cause_name == "current_producer":
            if profile.get("cause_binding") != "exact_current_proposal_revision_producer":
                fail(f"{prefix}: current-producer cause binding must be exact current producer", errors)
            if profile.get("action_class") != "producer_cause":
                fail(f"{prefix}: current-producer cause must be producer_cause", errors)
            if profile.get("admission") != "none":
                fail(f"{prefix}: current-producer cause must not use AuthorCommandAdmission", errors)
            if editor != {"session": "not-applicable", "writer_generation": "not-applicable", "takeover": "no"}:
                fail(f"{prefix}: current-producer cause cannot require editor session/generation", errors)
            if profile.get("idempotency") != "header_and_producer_cause_record":
                fail(f"{prefix}: current-producer cause idempotency must bind its producer cause", errors)
            if profile_recovery.get("requires_reconfirmation") != "forbidden":
                fail(f"{prefix}: current-producer cause must forbid RequiresReconfirmation", errors)
            if set(profile_settlement.get("acknowledgements", [])) != {"committed"}:
                fail(f"{prefix}: current-producer cause must acknowledge only committed", errors)
            if set(profile.get("producer_types", [])) != {"AgentRunStep", "ToolCall"}:
                fail(f"{prefix}: current-producer cause must bind AgentRunStep/ToolCall", errors)
            if "admission" in str(profile.get("semantic_cause", "")):
                fail(f"{prefix}: current-producer semantic cause must not mention an admission", errors)
    return profile_schema_ids


def validate_capability_coverage(
    catalog: dict[str, Any], operation_ids: set[str], errors: list[str]
) -> None:
    coverage = catalog.get("release1_capability_coverage")
    if not isinstance(coverage, dict):
        fail("release1_capability_coverage must be an object", errors)
        return
    declared_ids = coverage.get("required_capability_ids")
    if not isinstance(declared_ids, list) or set(declared_ids) != REQUIRED_RELEASE1_CAPABILITY_IDS or len(declared_ids) != len(set(declared_ids)):
        fail("Release 1 capability coverage must declare the complete required capability ID set", errors)

    capabilities = coverage.get("capabilities")
    if not isinstance(capabilities, list):
        fail("release1_capability_coverage.capabilities must be an array", errors)
        return
    by_id: dict[str, dict[str, Any]] = {}
    for capability in capabilities:
        if not isinstance(capability, dict):
            fail("Release 1 capability entries must be objects", errors)
            continue
        capability_id = capability.get("capability_id")
        if not isinstance(capability_id, str) or not capability_id:
            fail(f"invalid Release 1 capability ID: {capability_id!r}", errors)
            continue
        if capability_id in by_id:
            fail(f"duplicate Release 1 capability ID {capability_id}", errors)
        by_id[capability_id] = capability
        if not isinstance(capability.get("journey"), str) or not capability["journey"]:
            fail(f"{capability_id}: journey description is required", errors)
        required_operations = capability.get("required_operations")
        if not isinstance(required_operations, list) or not required_operations:
            fail(f"{capability_id}: required_operations must be a non-empty array", errors)
            required_operations = []
        if len(required_operations) != len(set(required_operations)):
            fail(f"{capability_id}: required_operations must not repeat an operation", errors)
        for operation_id in required_operations:
            if operation_id not in operation_ids:
                fail(f"{capability_id}: required capability operation is missing from catalog: {operation_id!r}", errors)

        operation_actions = capability.get("operation_actions", {})
        if not isinstance(operation_actions, dict):
            fail(f"{capability_id}: operation_actions must be an object", errors)
            operation_actions = {}
        unknown_action_operations = set(operation_actions) - set(required_operations)
        if unknown_action_operations:
            fail(f"{capability_id}: operation_actions reference non-required operations {sorted(unknown_action_operations)}", errors)
        for operation_id, actions in operation_actions.items():
            if not isinstance(actions, list) or not actions or any(not isinstance(action, str) or not action for action in actions):
                fail(f"{capability_id}.{operation_id}: action coverage must be a non-empty string array", errors)
            elif len(actions) != len(set(actions)):
                fail(f"{capability_id}.{operation_id}: action coverage must not repeat an action", errors)

    if set(by_id) != REQUIRED_RELEASE1_CAPABILITY_IDS:
        fail("Release 1 capability entries must exactly cover the required capability ID set", errors)

    exact_required_operations = {
        "human_readable_manuscript_export": {
            "exportHumanReadableManuscript",
            "getHumanReadableManuscriptExport",
        },
        "portable_project_archive": {
            "importProjectArchive",
            "exportProjectArchive",
            "getImportOperation",
            "getExportOperation",
        },
    }
    for capability_id, expected_operations in exact_required_operations.items():
        capability = by_id.get(capability_id)
        if capability is not None and set(capability.get("required_operations", [])) != expected_operations:
            fail(f"{capability_id}: export journey operation coverage is incomplete or conflated", errors)

    volume_capability = by_id.get("volume_chapter_lifecycle")
    if volume_capability is not None:
        actions = volume_capability.get("operation_actions", {})
        for operation_id in ("updateVolume", "updateChapter"):
            if set(actions.get(operation_id, [])) != {"rename", "reorder"}:
                fail(f"volume_chapter_lifecycle: {operation_id} must explicitly cover rename and reorder", errors)


def main() -> int:
    errors: list[str] = []
    raw = CATALOG_PATH.read_bytes()
    if b"\r" in raw:
        fail(f"{CATALOG_PATH}: CR byte found; catalog must use LF", errors)
    if not raw.endswith(b"\n"):
        fail(f"{CATALOG_PATH}: missing final LF", errors)

    try:
        catalog: dict[str, Any] = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        print(f"FAIL: invalid UTF-8/JSON: {exc}", file=sys.stderr)
        return 1

    if catalog.get("api_root") != "/api/v1":
        fail("catalog api_root must be /api/v1", errors)
    if catalog.get("compatibility_profile") != "storyos.public.same-release.v1":
        fail("catalog must select the sole same-release compatibility profile", errors)
    coverage = catalog.get("coverage_contract", {})
    if coverage.get("no_fixed_operation_count") is not True:
        fail("catalog must derive operation count rather than hard-code it", errors)
    if coverage.get("non_public_events_are_explicit") is not True:
        fail("catalog must explicitly classify non-public Event semantics", errors)

    owners = catalog.get("owners", {})
    if not isinstance(owners, dict):
        fail("owners must be an object", errors)
        owners = {}
    for owner_name, owner_source in owners.items():
        validate_source_reference(f"owner {owner_name!r}", owner_source, errors)

    protocol_text = PROTOCOL_PATH.read_text(encoding="utf-8")
    validate_source_reference("error_source", catalog.get("error_source"), errors)
    if "--self-test" in sys.argv:
        probe: list[str] = []
        missing_anchor = "__route_catalog_missing_anchor_self_test__"
        validate_source_reference(
            f"self-test {missing_anchor!r}",
            f"docs/foundation/versioned-command-query-artifact-event-protocol.md#{missing_anchor}",
            probe,
        )
        if not probe:
            fail("anchor negative self-test did not reject a missing anchor", errors)
    error_section = protocol_text.split("### 11.2 Required error semantics", 1)[-1].split("### 11.3", 1)[0]
    stable_error_rows = {
        code: int(status)
        for code, status in re.findall(
            r"^\|\s*`([^`]+)`\s*\|\s*(\d+)\s*\|", error_section, re.MULTILINE
        )
    }
    stable_errors = set(stable_error_rows)
    error_profiles = catalog.get("error_profiles", {})
    if not isinstance(error_profiles, dict):
        fail("error_profiles must be an object", errors)
        error_profiles = {}

    operations = catalog.get("operations", [])
    if not isinstance(operations, list) or not operations:
        fail("operations must be a non-empty array", errors)
        operations = []

    operation_ids: set[str] = set()
    route_keys: set[tuple[str, str]] = set()
    openapi_ids: set[str] = set()
    typescript_ids: set[str] = set()
    schema_id_owners: dict[str, str] = {}
    fixture_id_owners: dict[str, str] = {}
    event_kinds: dict[str, str] = {}
    release_identity = catalog.get("release_identity", {})
    release_identity_schema_id = release_identity.get("schema_id") if isinstance(release_identity, dict) else None
    if not isinstance(release_identity_schema_id, str) or not release_identity_schema_id:
        fail("release_identity.schema_id is required", errors)
    else:
        schema_id_owners[release_identity_schema_id] = "release_identity"
    event_catalog = catalog.get("events", [])
    event_by_id: dict[str, dict[str, Any]] = {}

    for event in event_catalog:
        if not isinstance(event, dict):
            fail("event catalog entries must be objects", errors)
            continue
        event_id = event.get("schema_id")
        if not isinstance(event_id, str) or not event_id.startswith("storyos.event."):
            fail(f"invalid Event schema ID: {event_id!r}", errors)
            continue
        if event_id in event_by_id:
            fail(f"duplicate Event schema ID: {event_id}", errors)
        event_by_id[event_id] = event
        previous_schema_owner = schema_id_owners.get(event_id)
        if previous_schema_owner is not None:
            fail(f"duplicate schema ID {event_id} ({previous_schema_owner} and Event)", errors)
        else:
            schema_id_owners[event_id] = f"Event {event_id}"
        if event.get("semantic_owner") not in owners:
            fail(f"Event {event_id} has unknown semantic owner", errors)
        event_kind = event.get("event_kind")
        if not isinstance(event_kind, str) or not event_kind:
            fail(f"Event {event_id} has no stable event_kind", errors)
        elif event_kind in event_kinds:
            existing_id = event_kinds[event_kind]
            existing_family = event_schema_family(existing_id, event_kind)
            new_family = event_schema_family(event_id, event_kind)
            if (
                existing_family is None
                or new_family is None
                or existing_family != new_family
            ):
                fail(
                    f"duplicate Event event_kind {event_kind} ({existing_id} and {event_id})",
                    errors,
                )
        else:
            event_kinds[event_kind] = event_id
        if event.get("wire_profile") != "storyos.project-activity.v1":
            fail(f"Event {event_id} must use the Release 1 Project Activity profile", errors)
        for fixture_key in ("positive_fixture", "negative_fixture"):
            fixture_id = event.get(fixture_key)
            if not isinstance(fixture_id, str):
                fail(f"invalid or duplicate Event fixture {fixture_id!r}", errors)
            elif fixture_id in fixture_id_owners:
                fail(f"duplicate fixture ID {fixture_id} ({fixture_id_owners[fixture_id]} and Event {event_id})", errors)
            else:
                fixture_id_owners[fixture_id] = f"Event {event_id}"

    for operation in operations:
        if not isinstance(operation, dict):
            fail("operation catalog entries must be objects", errors)
            continue
        required = {
            "operation_id",
            "kind",
            "method",
            "path",
            "shape",
            "semantic_owner",
            "wire_owner",
            "schemas",
            "action_class",
            "admission",
            "editor",
            "idempotency",
            "preconditions",
            "settlement",
            "error_profile",
            "activity",
            "fixtures",
            "generated",
        }
        missing = required - operation.keys()
        if missing:
            fail(f"operation {operation.get('operation_id', '<missing>')} missing {sorted(missing)}", errors)
            continue

        operation_id = operation["operation_id"]
        method = operation["method"]
        path = operation["path"]
        if not isinstance(operation_id, str) or not re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", operation_id):
            fail(f"invalid operation_id {operation_id!r}", errors)
        elif operation_id in operation_ids:
            fail(f"duplicate operation_id {operation_id}", errors)
        else:
            operation_ids.add(operation_id)

        if operation.get("kind") not in ALLOWED_KINDS:
            fail(f"{operation_id}: unknown operation kind {operation.get('kind')!r}", errors)
        if method not in ALLOWED_METHODS:
            fail(f"{operation_id}: unknown HTTP method {method!r}", errors)
        if not isinstance(path, str) or not path.startswith("/api/v1/"):
            fail(f"{operation_id}: path must be rooted at /api/v1", errors)
        route_key = (method, path)
        if route_key in route_keys:
            fail(f"duplicate method/path {method} {path}", errors)
        route_keys.add(route_key)
        if "?" in path:
            fail(f"{operation_id}: query parameters must be schema/profile fields, not route identity", errors)
        if operation.get("semantic_owner") not in owners or operation.get("wire_owner") not in owners:
            fail(f"{operation_id}: semantic/wire owner is not in owners", errors)
        if "action_coverage" in operation:
            action_coverage = operation.get("action_coverage")
            if not isinstance(action_coverage, list) or not action_coverage or any(not isinstance(action, str) or not action for action in action_coverage):
                fail(f"{operation_id}: action_coverage must be a non-empty string array", errors)
            elif len(action_coverage) != len(set(action_coverage)):
                fail(f"{operation_id}: action_coverage must not repeat an action", errors)
        if operation_id in {"updateVolume", "updateChapter"} and operation.get("action_coverage") != ["rename", "reorder"]:
            fail(f"{operation_id}: rename and reorder action coverage must be explicit and ordered", errors)

        schemas = operation["schemas"]
        schema_map = schemas if isinstance(schemas, dict) else {}
        generated = operation["generated"] if isinstance(operation["generated"], dict) else {}
        cause_schema_ids = validate_conditional_cause_profiles(
            operation, owners, error_profiles, stable_error_rows, errors
        )
        if not isinstance(schemas, dict) or not isinstance(schemas.get("request"), str) or not isinstance(schemas.get("response"), str):
            fail(f"{operation_id}: request/response schema IDs are required", errors)
        else:
            schema_ids = generated.get("json_schema_ids", [])
            expected_schema_ids = {schemas["request"], schemas["response"], *cause_schema_ids}
            if not isinstance(schema_ids, list) or set(schema_ids) != expected_schema_ids:
                fail(f"{operation_id}: generated JSON Schema coverage must equal all operation schema IDs", errors)
            if isinstance(schema_ids, list) and len(schema_ids) != len(set(schema_ids)):
                fail(f"{operation_id}: generated JSON Schema IDs must be unique", errors)
            if schemas["request"] == schemas["response"]:
                fail(f"{operation_id}: request and response schema IDs must be distinct", errors)
            if isinstance(schema_ids, list):
                for schema_id in schema_ids:
                    if not isinstance(schema_id, str):
                        fail(f"{operation_id}: generated JSON Schema IDs must be strings", errors)
                        continue
                    previous_schema_owner = schema_id_owners.get(schema_id)
                    if previous_schema_owner is not None:
                        fail(f"duplicate schema ID {schema_id} ({previous_schema_owner} and {operation_id})", errors)
                    else:
                        schema_id_owners[schema_id] = operation_id
        if schema_map.get("request_source") not in owners or schema_map.get("response_source") not in owners:
            fail(f"{operation_id}: schema source mapping is incomplete", errors)
        if schema_map.get("semantic_source") != operation.get("semantic_owner"):
            fail(f"{operation_id}: schema semantic source must match semantic_owner", errors)

        editor = operation["editor"]
        if not isinstance(editor, dict) or not {"session", "writer_generation", "takeover"} <= editor.keys():
            fail(f"{operation_id}: editor session/writer-generation/takeover rule is incomplete", errors)
        if not isinstance(operation["preconditions"], list) or not operation["preconditions"]:
            fail(f"{operation_id}: preconditions must be explicit and non-empty", errors)

        settlement = operation["settlement"]
        if not isinstance(settlement, dict) or not isinstance(settlement.get("http_statuses"), list) or not settlement["http_statuses"]:
            fail(f"{operation_id}: settlement/status profile is incomplete", errors)
        else:
            if any(not isinstance(status, int) for status in settlement["http_statuses"]):
                fail(f"{operation_id}: HTTP statuses must be integers", errors)
            if "accepted" in settlement.get("acknowledgements", []):
                if (
                    not settlement.get("settlement_query")
                    or not settlement.get("settlement_query_operation_id")
                    or not settlement.get("operation_ref")
                ):
                    fail(f"{operation_id}: Accepted requires an inspectable settlement query operation and reference", errors)
        if operation.get("error_profile") not in error_profiles:
            fail(f"{operation_id}: unknown error profile {operation.get('error_profile')!r}", errors)
        else:
            unknown_errors = set(error_profiles[operation["error_profile"]]) - stable_errors
            if unknown_errors:
                fail(f"{operation_id}: error profile contains codes absent from protocol section 11.2: {sorted(unknown_errors)}", errors)
            missing_error_statuses = {
                stable_error_rows[code]
                for code in error_profiles[operation["error_profile"]]
                if code in stable_error_rows
            } - set(settlement.get("http_statuses", []) if isinstance(settlement, dict) else [])
            if missing_error_statuses:
                fail(f"{operation_id}: settlement HTTP statuses omit error-profile statuses {sorted(missing_error_statuses)}", errors)

        activity = operation["activity"]
        if not isinstance(activity, dict) or not {"success", "no_effect", "pre_admission"} <= activity.keys():
            fail(f"{operation_id}: Project Activity mapping is incomplete", errors)
        else:
            for event_id in [*activity.get("success", []), *activity.get("no_effect", [])]:
                if event_id not in event_by_id:
                    fail(f"{operation_id}: missing Event schema catalog entry {event_id}", errors)
            if activity.get("pre_admission") != "none":
                fail(f"{operation_id}: pre-admission activity must be none", errors)
            if operation.get("kind") != "command" and (activity.get("success") or activity.get("no_effect")):
                fail(f"{operation_id}: non-command operation cannot produce command Activity mapping", errors)

        fixtures = operation["fixtures"]
        if not isinstance(fixtures, dict) or not isinstance(fixtures.get("positive"), str) or not isinstance(fixtures.get("negative"), list) or not fixtures["negative"]:
            fail(f"{operation_id}: positive and negative golden fixture IDs are required", errors)
        else:
            for fixture_id in [fixtures["positive"], *fixtures["negative"]]:
                if fixture_id in fixture_id_owners:
                    fail(f"duplicate fixture ID {fixture_id} ({fixture_id_owners[fixture_id]} and {operation_id})", errors)
                else:
                    fixture_id_owners[fixture_id] = operation_id

        if generated.get("openapi_operation_id") != operation_id:
            fail(f"{operation_id}: OpenAPI operation ID mismatch", errors)
        if generated.get("typescript_operation") != operation_id:
            fail(f"{operation_id}: TypeScript operation mismatch", errors)
        if generated.get("schema_catalog") != "storyos.schema-catalog.v1":
            fail(f"{operation_id}: schema catalog identity mismatch", errors)
        if generated.get("openapi") is not True or generated.get("typescript_client") is not True or generated.get("golden_wire_corpus") is not True:
            fail(f"{operation_id}: generated coverage flags are incomplete", errors)
        if operation_id in openapi_ids:
            fail(f"duplicate OpenAPI operation ID {operation_id}", errors)
        if operation_id in typescript_ids:
            fail(f"duplicate TypeScript operation {operation_id}", errors)
        openapi_ids.add(operation_id)
        typescript_ids.add(operation_id)

        if operation.get("action_class") in EDITOR_ACTIONS:
            if "accepted" in (settlement.get("acknowledgements", []) if isinstance(settlement, dict) else []):
                fail(f"{operation_id}: Release 1 editor commands must not return Accepted", errors)
            if not isinstance(settlement, dict) or settlement.get("mode") != "sync":
                fail(f"{operation_id}: Release 1 editor commands must settle synchronously", errors)

    operation_by_id = {
        operation.get("operation_id"): operation
        for operation in operations
        if isinstance(operation, dict) and isinstance(operation.get("operation_id"), str)
    }
    for operation in operations:
        if not isinstance(operation, dict):
            continue
        settlement = operation.get("settlement", {})
        if not isinstance(settlement, dict) or "accepted" not in settlement.get("acknowledgements", []):
            continue
        operation_id = operation.get("operation_id", "<missing>")
        query_operation_id = settlement.get("settlement_query_operation_id")
        query_operation = operation_by_id.get(query_operation_id)
        if not isinstance(query_operation, dict):
            fail(f"{operation_id}: settlement query operation does not exist: {query_operation_id!r}", errors)
            continue
        if query_operation.get("kind") != "query" or query_operation.get("method") != "GET":
            fail(f"{operation_id}: settlement query operation must be a GET query: {query_operation_id}", errors)
        if query_operation.get("path") != settlement.get("settlement_query"):
            fail(f"{operation_id}: settlement query operation path does not match settlement_query", errors)

    validate_capability_coverage(catalog, operation_ids, errors)

    author_edit = [op for op in operations if op.get("operation_id") == "applyAuthorEdit"]
    if len(author_edit) != 1:
        fail("catalog must contain exactly one applyAuthorEdit", errors)
    else:
        op = author_edit[0]
        if op.get("method") != "POST" or op.get("path") != "/api/v1/projects/{project_id}/manuscript/author-edits":
            fail("applyAuthorEdit is not the sole canonical author-edit ingress", errors)
        if op.get("action_class") != "direct_editor_action" or op.get("admission") != "author_command":
            fail("applyAuthorEdit must be a directly admitted ownership-classifying command", errors)
        if "Core classifies" not in op.get("notes", "") or "Web Client never chooses" not in op.get("notes", ""):
            fail("applyAuthorEdit notes must state the no-client-route-selection boundary", errors)
        settlement = op.get("settlement", {})
        if settlement.get("settlement_query") is not None or settlement.get("settlement_query_operation_id") is not None:
            fail("applyAuthorEdit must not claim the acknowledgement-loss bootstrap Query as an Accepted settlement link", errors)

    outcome_queries = [
        op for op in operations if op.get("operation_id") == "getApplyAuthorEditOutcome"
    ]
    if len(outcome_queries) != 1:
        fail("catalog must contain exactly one getApplyAuthorEditOutcome", errors)
    else:
        outcome_query = outcome_queries[0]
        if (
            outcome_query.get("kind") != "query"
            or outcome_query.get("method") != "GET"
            or outcome_query.get("path")
            != "/api/v1/projects/{project_id}/manuscript/author-edit-outcomes/{idempotency_key}"
        ):
            fail("getApplyAuthorEditOutcome must be the exact named GET route", errors)
        expected_proof = {
            "identity": "apply_author_edit_idempotency_key",
            "header": "X-StoryOS-Anti-Forgery",
            "source": "original_project_command_challenge_nonce",
            "url": "forbidden",
            "response": "forbidden",
            "consumption": "none",
            "cache_control": "no-store",
        }
        if outcome_query.get("query_proof") != expected_proof:
            fail("getApplyAuthorEditOutcome query proof must be header-only, non-consuming, and non-cacheable", errors)
        if outcome_query.get("editor", {}).get("writer_generation") != "bound-to-original-admission-when-present":
            fail("getApplyAuthorEditOutcome may require the original writer generation only after Admission", errors)
        settlement = outcome_query.get("settlement", {})
        if (
            settlement.get("mode") != "read"
            or settlement.get("acknowledgements") != []
            or settlement.get("settlement_query") is not None
            or settlement.get("operation_ref") is not None
        ):
            fail("getApplyAuthorEditOutcome must remain an independent read, not an Accepted settlement link", errors)
        notes = outcome_query.get("notes", "")
        for boundary in ("Rejected requires", "StillUnknown grants no retry", "creates no Admission"):
            if boundary not in notes:
                fail(f"getApplyAuthorEditOutcome notes omit boundary: {boundary}", errors)

    for non_public in catalog.get("non_public_operations", []):
        if non_public.get("public_route") is not None:
            fail(f"non-public operation {non_public.get('name')} has a public route", errors)
        if non_public.get("source") not in owners:
            fail(f"non-public operation {non_public.get('name')} has unknown source owner", errors)

    for non_public in catalog.get("non_public_events", []):
        if non_public.get("public_schema") is not None:
            fail(f"non-public event {non_public.get('name')} has a public schema", errors)
        if non_public.get("source") not in owners:
            fail(f"non-public event {non_public.get('name')} has unknown source owner", errors)

    referenced_events = set()
    for operation in operations:
        activity = operation.get("activity", {})
        if isinstance(activity, dict):
            referenced_events.update(activity.get("success", []))
            referenced_events.update(activity.get("no_effect", []))
    for event_id in sorted(set(event_by_id) - referenced_events):
        fail(f"public Event schema {event_id} is not mapped from any catalog operation", errors)

    if errors:
        print("FAIL:")
        for error in errors:
            print(f"- {error}")
        return 1

    by_kind = {kind: sum(1 for op in operations if op["kind"] == kind) for kind in sorted(ALLOWED_KINDS)}
    print(f"OK: {len(operations)} operations ({by_kind}); {len(event_by_id)} Event schemas; no fixed count asserted")
    if "--self-test" in sys.argv:
        print("OK: missing-anchor negative self-test rejected a nonexistent anchor")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
