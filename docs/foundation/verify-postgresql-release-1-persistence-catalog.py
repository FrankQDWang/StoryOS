#!/usr/bin/env python3
"""Public review-time entry point for the Release 1 PostgreSQL catalog.

The command and output contract intentionally remain stable.  Validation axes
live in sibling modules so storage, route, and self-test changes stay focused.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

# Keep sibling modules importable both from the public script command and from
# review tooling that loads this hyphenated entry file by path.
ENTRY_DIR = str(Path(__file__).resolve().parent)
if ENTRY_DIR not in sys.path:
    sys.path.insert(0, ENTRY_DIR)

from postgresql_persistence_verifier_common import (
    CATALOG_PATH,
    ROOT,
    ROUTE_CATALOG_PATH,
    STORAGE_CONTRACT_PATH,
    canonical_bytes,
    fail,
    normalized_file_bytes,
    validate_relative_links,
    validate_source,
)
from postgresql_persistence_verifier_route import validate_route_coverage
from postgresql_persistence_verifier_storage import (
    validate_families,
    validate_author_command_outcome_unknown_sql,
    validate_migration,
    validate_physical_ledger_boundary,
)


def validate_catalog(catalog: dict[str, Any], route_catalog: dict[str, Any], errors: list[str], check_digests: bool = True) -> dict[str, int]:
    if catalog.get("catalog_id") != catalog.get("schema_identity", {}).get("persisted_format_catalog_id"):
        fail(errors, "catalog identity does not bind persisted-format identity")
    if catalog.get("catalog_id") != "storyos.persistence.catalog.release-1.v3":
        fail(errors, "catalog identity must equal the hard-cut Release 1 catalog v3")
    if catalog.get("contract_revision") != "release1-storage-contract-2026-08-17-author-command-outcome-unknown":
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
    validate_physical_ledger_boundary(catalog, family_map, errors)
    route_stats = validate_route_coverage(catalog, route_catalog, family_map, operation_owners, event_owners, errors)
    verification_contract = catalog.get("verification_contract", {})
    required_verifier_flags = {
        "require_author_command_outcome_unknown",
        "require_negative_self_test",
        "require_named_author_edit_outcome_query_read",
        "require_physical_ledger_boundary",
        "require_bootstrap_source_checksum",
        "require_table_family_registry_digest",
        "require_route_settlement_and_activity_coverage",
    }
    for flag in required_verifier_flags:
        if verification_contract.get(flag) is not True:
            fail(errors, f"catalog must require verifier flag {flag}")
    return {
        "family_count": len(family_map),
        "table_family_count": sum(len(family.get("table_families", [])) for family in family_map.values()),
        "project_scoped_family_count": sum(1 for family in family_map.values() if family.get("scope", {}).get("requires_project_scope") is True),
        "projection_family_count": sum(1 for family in family_map.values() if family.get("authority_class") == "projection"),
        **route_stats,
    }


def run_negative_self_tests(catalog: dict[str, Any], route_catalog: dict[str, Any], errors: list[str]) -> None:
    catalog_identity_probe = copy.deepcopy(catalog)
    catalog_identity_probe["catalog_id"] = "storyos.persistence.catalog.release-1.v4"
    catalog_identity_probe["schema_identity"]["persisted_format_catalog_id"] = (
        "storyos.persistence.catalog.release-1.v4"
    )
    catalog_identity_errors: list[str] = []
    validate_catalog(
        catalog_identity_probe,
        route_catalog,
        catalog_identity_errors,
        check_digests=False,
    )
    if not any("hard-cut Release 1 catalog v3" in error for error in catalog_identity_errors):
        errors.append("negative self-test did not reject paired catalog identity drift")

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

    settlement_probe = copy.deepcopy(catalog)
    settlement_family = next(family for family in settlement_probe["families"] if family["family_id"] == "project-canonical")
    settlement_family["public_contract"]["settlement_modes"] = ["nonsense"]
    settlement_errors: list[str] = []
    validate_catalog(settlement_probe, route_catalog, settlement_errors, check_digests=False)
    if not any("settlement_modes" in error for error in settlement_errors):
        errors.append("negative self-test did not reject an invalid settlement mode binding")

    portability_probe = copy.deepcopy(catalog)
    portability_family = next(family for family in portability_probe["families"] if family["family_id"] == "project-canonical")
    portability_family["portability"]["physical_backup"] = "exclude_everything"
    portability_family["portability"]["whole_service_restore"] = "discard_canonical_rows"
    portability_errors: list[str] = []
    validate_catalog(portability_probe, route_catalog, portability_errors, check_digests=False)
    if not any("portability" in error for error in portability_errors):
        errors.append("negative self-test did not reject invalid portability classifications")

    predecessor_probe = copy.deepcopy(catalog)
    predecessor_probe["families"][0]["schema"]["supported_predecessors"] = ["invented.predecessor.v1"]
    predecessor_errors: list[str] = []
    validate_catalog(predecessor_probe, route_catalog, predecessor_errors, check_digests=False)
    if not any("supported predecessors" in error for error in predecessor_errors):
        errors.append("negative self-test did not reject a family predecessor-set bypass")

    table_probe = copy.deepcopy(catalog)
    table_probe["families"][0]["table_families"][0] = "totally_invented_table_family"
    table_errors: list[str] = []
    validate_catalog(table_probe, route_catalog, table_errors, check_digests=False)
    if not any("table-family registry digest mismatch" in error for error in table_errors):
        errors.append("negative self-test did not reject an invented table family")

    bootstrap_probe = copy.deepcopy(catalog)
    bootstrap_probe["migration_chain"]["bootstrap"]["sources"][0]["lf_sha256"] = "sha256:" + "0" * 64
    bootstrap_errors: list[str] = []
    validate_catalog(bootstrap_probe, route_catalog, bootstrap_errors, check_digests=False)
    if not any("bootstrap SQL checksum mismatch" in error for error in bootstrap_errors):
        errors.append("negative self-test did not reject bootstrap SQL checksum drift")

    receipt_probe = copy.deepcopy(catalog)
    receipt_probe["author_edit_receipt_activity"]["zero_relation_cardinality"] = "fake_activity"
    receipt_errors: list[str] = []
    validate_catalog(receipt_probe, route_catalog, receipt_errors, check_digests=False)
    if not any("Receipt/Activity contract" in error for error in receipt_errors):
        errors.append("negative self-test did not reject Receipt/Activity contract drift")

    nullable_receipt_probe = copy.deepcopy(catalog)
    nullable_receipt_probe["author_edit_receipt_activity"]["typed_value_guard"] = (
        "allow_check_unknown"
    )
    nullable_receipt_errors: list[str] = []
    validate_catalog(
        nullable_receipt_probe,
        route_catalog,
        nullable_receipt_errors,
        check_digests=False,
    )
    if not any("Receipt/Activity contract" in error for error in nullable_receipt_errors):
        errors.append("negative self-test did not reject nullable Receipt contract drift")

    credential_probe = copy.deepcopy(catalog)
    credential_probe["migration_chain"]["bootstrap"]["runtime_credential_source"] = (
        "tracked_bootstrap_password"
    )
    credential_errors: list[str] = []
    validate_catalog(credential_probe, route_catalog, credential_errors, check_digests=False)
    if not any("runtime credential" in error for error in credential_errors):
        errors.append("negative self-test did not reject tracked runtime credential ownership")

    outcome_query_probe = copy.deepcopy(catalog)
    outcome_activity_family = next(
        family
        for family in outcome_query_probe["families"]
        if family["family_id"] == "operational-project-activity"
    )
    outcome_activity_family["public_contract"]["operation_ids"].remove(
        "getApplyAuthorEditOutcome"
    )
    outcome_query_errors: list[str] = []
    validate_catalog(
        outcome_query_probe,
        route_catalog,
        outcome_query_errors,
        check_digests=False,
    )
    if not any("getApplyAuthorEditOutcome must map exactly" in error for error in outcome_query_errors):
        errors.append("negative self-test did not reject incomplete outcome Query family coverage")

    unknown_mutations = [
        (
            "missing-table-family",
            lambda probe: next(
                family
                for family in probe["families"]
                if family["family_id"] == "operational-admission-editor"
            )["table_families"].remove(
                "author_command_admission_outcome_unknown_observations"
            ),
        ),
        (
            "false-reconciliation",
            lambda probe: probe["author_command_outcome_unknown"].__setitem__(
                "reconciliation_required", "boolean"
            ),
        ),
        (
            "open-boundary",
            lambda probe: probe["author_command_outcome_unknown"].__setitem__(
                "last_provable_boundaries", ["admission_committed", "network_missing"]
            ),
        ),
        (
            "open-reason",
            lambda probe: probe["author_command_outcome_unknown"].__setitem__(
                "reasons", ["acknowledgement_missing", "unknown"]
            ),
        ),
        (
            "global-observation-identity",
            lambda probe: probe["author_command_outcome_unknown"].__setitem__(
                "row_identity", ["observation_id"]
            ),
        ),
        (
            "unbounded-sequence",
            lambda probe: probe["author_command_outcome_unknown"].__setitem__(
                "sequence", "unbounded"
            ),
        ),
        (
            "post-terminal-append",
            lambda probe: probe["author_command_outcome_unknown"].__setitem__(
                "new_after_terminal", "append"
            ),
        ),
    ]
    for name, mutate in unknown_mutations:
        unknown_probe = copy.deepcopy(catalog)
        mutate(unknown_probe)
        unknown_errors: list[str] = []
        validate_catalog(
            unknown_probe,
            route_catalog,
            unknown_errors,
            check_digests=False,
        )
        if not any("outcome_unknown" in error for error in unknown_errors):
            errors.append(f"negative self-test did not reject {name} outcome_unknown drift")

    outcome_sql = normalized_file_bytes(
        ROOT / "crates/storyos-adapter-postgres/migrations/0005_author_edits.sql"
    ).decode("utf-8")
    sql_mutations = [
        (
            "nullable-reconciliation-column",
            lambda value: value.replace(
                "reconciliation_required boolean NOT NULL DEFAULT true",
                "reconciliation_required boolean DEFAULT true",
                1,
            ),
        ),
        (
            "nullable-boundary-column",
            lambda value: value.replace(
                "last_provable_boundary text NOT NULL",
                "last_provable_boundary text",
                1,
            ),
        ),
        (
            "nonliteral-reconciliation-column",
            lambda value: value.replace(
                "CHECK (reconciliation_required)",
                "CHECK (true)",
                1,
            ),
        ),
        (
            "wrong-arbiter-lock",
            lambda value: value.replace(
                "FROM storyos.command_idempotency\n"
                "   WHERE owner_user_id = NEW.owner_user_id",
                "FROM storyos.project_command_challenges\n"
                "   WHERE owner_user_id = NEW.owner_user_id",
                1,
            ),
        ),
        (
            "caller-command-identity",
            lambda value: value.replace(
                "NEW.command_id := admission.command_id;",
                "NEW.command_id := NEW.command_id;",
                1,
            ),
        ),
        (
            "missing-in-progress-guard",
            lambda value: value.replace(
                "IF NOT FOUND OR arbiter_outcome_kind <> 'in_progress' THEN",
                "IF NOT FOUND THEN",
                1,
            ),
        ),
        (
            "missing-terminal-guard",
            lambda value: re.sub(
                r"  IF EXISTS \(\n"
                r"    SELECT 1\n"
                r"      FROM storyos\.author_command_admission_settlements\n"
                r"     WHERE owner_user_id = NEW\.owner_user_id\n"
                r"       AND project_id = NEW\.project_id\n"
                r"       AND author_command_admission_id = NEW\.author_command_admission_id\n"
                r"  \) THEN\n"
                r"    RAISE EXCEPTION USING\n"
                r"      ERRCODE = '23514',\n"
                r"      MESSAGE = 'a terminal Admission cannot append outcome_unknown';\n"
                r"  END IF;\n",
                "",
                value,
                count=1,
            ),
        ),
        (
            "caller-sequence",
            lambda value: value.replace(
                "NEW.observation_sequence := next_sequence;",
                "NEW.observation_sequence := NEW.observation_sequence;",
                1,
            ),
        ),
        (
            "caller-observed-time",
            lambda value: value.replace(
                "NEW.observed_at := clock_timestamp();",
                "NEW.observed_at := NEW.observed_at;",
                1,
            ),
        ),
        (
            "missing-admission-foreign-key",
            lambda value: value.replace(
                "  FOREIGN KEY (\n"
                "    owner_user_id, project_id, author_command_admission_id, command_id,\n"
                "    command_kind, canonical_command_digest, idempotency_key\n"
                "  ) REFERENCES storyos.author_command_admissions (\n"
                "    owner_user_id, project_id, author_command_admission_id, command_id,\n"
                "    command_kind, canonical_command_digest, idempotency_key\n"
                "  ) MATCH FULL\n",
                "",
                1,
            ),
        ),
        (
            "permissive-scope-policy",
            lambda value: value.replace(
                "  USING (\n"
                "    owner_user_id = current_setting('storyos.owner_user_id')::uuid\n"
                "    AND project_id = current_setting('storyos.project_id')::uuid\n"
                "  )",
                "  USING (true)",
                1,
            ),
        ),
        (
            "indirect-runtime-update-grant",
            lambda value: value.replace(
                "GRANT SELECT, INSERT ON storyos.author_command_admissions,",
                "GRANT UPDATE ON storyos.author_command_admissions,\n"
                "  storyos.author_command_admission_outcome_unknown_observations "
                "TO storyos_runtime;\n\n"
                "GRANT SELECT, INSERT ON storyos.author_command_admissions,",
                1,
            ),
        ),
    ]
    for name, mutate in sql_mutations:
        mutated_sql = mutate(outcome_sql)
        if mutated_sql == outcome_sql:
            errors.append(f"negative self-test could not apply {name} SQL mutation")
            continue
        sql_errors: list[str] = []
        validate_author_command_outcome_unknown_sql(mutated_sql, sql_errors)
        if not sql_errors:
            errors.append(f"negative self-test did not reject {name} outcome_unknown SQL drift")

    outcome_read_mutations = [
        (
            "missing-family",
            lambda relation: relation["family_ids"].remove("project-canonical"),
        ),
        (
            "surplus-family",
            lambda relation: relation["family_ids"].append("operational-run-mailbox"),
        ),
        (
            "write-capability",
            lambda relation: relation["capabilities"].append("write"),
        ),
        (
            "lifecycle-append",
            lambda relation: relation.__setitem__("lifecycle_append", "outcome_unknown"),
        ),
        (
            "observation-table-read",
            lambda relation: relation.__setitem__("outcome_unknown_table_access", "read"),
        ),
    ]
    for name, mutate in outcome_read_mutations:
        read_probe = copy.deepcopy(catalog)
        mutate(read_probe["author_edit_outcome_query_read"])
        read_errors: list[str] = []
        validate_catalog(read_probe, route_catalog, read_errors, check_digests=False)
        if not any("named read relation" in error for error in read_errors):
            errors.append(f"negative self-test did not reject {name} outcome Query drift")


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
        run_negative_self_tests(catalog, route_catalog, errors)
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
        print("OK: paired catalog-identity negative self-test rejected v3 drift")
        print("OK: duplicate-family negative self-test rejected a bad catalog")
        print("OK: missing-owner-anchor negative self-test rejected a bad catalog")
        print("OK: settlement-mode negative self-test rejected a bad catalog")
        print("OK: portability negative self-test rejected a bad catalog")
        print("OK: predecessor-set negative self-test rejected a bad catalog")
        print("OK: invented-table-family negative self-test rejected a bad catalog")
        print("OK: bootstrap-checksum negative self-test rejected a bad catalog")
        print("OK: Receipt/Activity negative self-test rejected a bad catalog")
        print("OK: nullable-Receipt negative self-test rejected a bad catalog")
        print("OK: bootstrap-credential negative self-test rejected a bad catalog")
        print("OK: outcome-Query-family negative self-test rejected a bad catalog")
        print("OK: outcome_unknown negative self-tests rejected seven bad catalogs and twelve bad SQL shapes")
        print("OK: named-outcome-read negative self-tests rejected five bad catalogs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
