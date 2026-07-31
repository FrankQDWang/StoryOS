"""Shared constants and source/catalog helpers for the PostgreSQL verifier.

This module has no storage- or route-specific validation.  It owns only the
normalization, source-link, selector, digest, and graph primitives shared by
the focused verifier modules and the public entry script.
"""

from __future__ import annotations

import json
import re
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
ALLOWED_SETTLEMENT_MODES = {"challenge", "sync", "async", "conditional_by_cause", "read", "stream"}
ALLOWED_WIRE_HISTORY = {"not_applicable", "referenced_by_wire_history", "canonical", "event_representation_referenced"}
ALLOWED_PORTABILITY = {
    "physical_backup": {"full_rows", "full_rows_and_external_chain_manifest", "full_rows_without_secret_values", "metadata_only"},
    "whole_service_restore": {
        "rebuild",
        "rebuild_or_restore_and_verify",
        "rebuild_or_unavailable",
        "restore_and_mark_unbound",
        "restore_and_reconcile_outcome_unknown",
        "restore_and_verify",
        "restore_and_verify_before_visibility",
        "restore_in_isolation",
        "restore_metadata_then_rebuild",
    },
    "project_export": {
        "archive_manifest_and_entries",
        "exclude",
        "include",
        "include_if_required_for_historical_evidence",
        "include_nonsecret_evidence",
        "include_reference_only",
        "not_applicable",
        "reference_by_identity",
        "reference_only",
    },
    "project_restore": {
        "authorize_same_user",
        "not_applicable",
        "rebuild",
        "rebuild_from_canonical",
        "resolve_or_fail",
        "restore_and_unbound_credentials",
        "restore_exact_scope",
        "stage_validate_promote_atomically",
    },
    "secret_exclusion": {"no_secret_material", "reference_only"},
}


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
