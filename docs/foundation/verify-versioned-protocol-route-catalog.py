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


def fail(message: str, errors: list[str]) -> None:
    errors.append(message)


def source_path(source: str) -> Path:
    return ROOT / source.split("#", 1)[0]


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
    for owner_name, owner_source in owners.items():
        if not isinstance(owner_source, str) or not source_path(owner_source).is_file():
            fail(f"owner {owner_name!r} points to a missing tracked source: {owner_source!r}", errors)
        if ".reference" in owner_source:
            fail(f"owner {owner_name!r} points into .reference", errors)

    protocol_text = PROTOCOL_PATH.read_text(encoding="utf-8")
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
    operation_fixture_ids: set[str] = set()
    event_catalog = catalog.get("events", [])
    event_by_id: dict[str, dict[str, Any]] = {}
    event_fixture_ids: set[str] = set()

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
        if event.get("semantic_owner") not in owners:
            fail(f"Event {event_id} has unknown semantic owner", errors)
        if not isinstance(event.get("event_kind"), str) or not event["event_kind"]:
            fail(f"Event {event_id} has no stable event_kind", errors)
        if event.get("wire_profile") != "storyos.project-activity.v1":
            fail(f"Event {event_id} must use the Release 1 Project Activity profile", errors)
        for fixture_key in ("positive_fixture", "negative_fixture"):
            fixture_id = event.get(fixture_key)
            if not isinstance(fixture_id, str) or fixture_id in event_fixture_ids:
                fail(f"invalid or duplicate Event fixture {fixture_id!r}", errors)
            else:
                event_fixture_ids.add(fixture_id)

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

        schemas = operation["schemas"]
        schema_map = schemas if isinstance(schemas, dict) else {}
        generated = operation["generated"] if isinstance(operation["generated"], dict) else {}
        if not isinstance(schemas, dict) or not isinstance(schemas.get("request"), str) or not isinstance(schemas.get("response"), str):
            fail(f"{operation_id}: request/response schema IDs are required", errors)
        else:
            schema_ids = generated.get("json_schema_ids", [])
            if set(schema_ids) != {schemas["request"], schemas["response"]}:
                fail(f"{operation_id}: generated JSON Schema coverage must equal request and response IDs", errors)
            if schemas["request"] == schemas["response"]:
                fail(f"{operation_id}: request and response schema IDs must be distinct", errors)
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
                if not settlement.get("settlement_query") or not settlement.get("operation_ref"):
                    fail(f"{operation_id}: Accepted requires an inspectable settlement query and operation reference", errors)
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
                if fixture_id in operation_fixture_ids:
                    fail(f"duplicate operation fixture ID {fixture_id}", errors)
                operation_fixture_ids.add(fixture_id)

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
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
