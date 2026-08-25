#!/usr/bin/env python3
"""Validate the structured prerelease policy and its evidence/document bindings.

This is a contract-source and projection-drift check. The browser harness is
the executable semantic oracle; this verifier does not infer semantics from
prose or claim that production code already conforms.
"""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import re
import sys
ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = ROOT / "docs/foundation/author-edit-batch-release-1-policy.json"
EVIDENCE_PATH = ROOT / "docs/research/author-edit-batch-prerelease-browser-evidence.json"
CANDIDATES_PATH = ROOT / "docs/research/author-edit-batch-prerelease-browser-candidates.jsonl"
APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_PATH = (
    ROOT / "generated/json-schema/storyos-public-release-1/apply-author-edit-response.schema.json"
)
APPLY_AUTHOR_EDIT_OUTCOME_SCHEMA_PATH = (
    ROOT
    / "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-response.schema.json"
)
ROUTE_CATALOG_PATH = ROOT / "docs/foundation/versioned-protocol-release-1-route-catalog.json"
TYPESCRIPT_CLIENT_PATH = ROOT / "generated/typescript/storyos-public-release-1/client.d.mts"
AUTHOR_ADMISSION_PATH = ROOT / "docs/foundation/author-command-admission.md"
PROJECTIONS = (
    ROOT / "CONTEXT.md",
    ROOT / "docs/adr/0003-specify-manuscript-revision-and-proposal-state-machine.md",
    ROOT / "docs/foundation/manuscript-revision-proposal-state-machine.md",
    ROOT / "docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md",
    ROOT / "docs/foundation/deterministic-verification-and-failure-recovery-gates.md",
    AUTHOR_ADMISSION_PATH,
)
REVISION = "storyos.author-edit-batch.release-1.preview.v1"
EDITOR_REVISION = "storyos.editor-contract.release-1.v2"
PROJECTION_REQUIREMENTS = {
    PROJECTIONS[0]: ("**Editor Verification Split**:", "typed zero-authority Receipt",
                     "pre-Admission refusal or infrastructure failure before commit"),
    PROJECTIONS[1]: ("#70-owned", "author-edit-batch-release-1-policy.json", REVISION,
                     EDITOR_REVISION, "240-intent and 250 ms observations",
                     "typed zero-authority Receipt"),
    PROJECTIONS[2]: ("#70-owned structured", "author-edit-batch-release-1-policy.json",
                     REVISION, EDITOR_REVISION, "Web proves", "Core receives",
                     "typed zero-authority Receipt"),
    PROJECTIONS[3]: ("#70-owned", "author-edit-batch-release-1-policy.json",
                     EDITOR_REVISION, "240 consecutive typing"),
    PROJECTIONS[4]: ("`DVG-01`", "`DVG-02`", "`DVG-03`", "#70-owned structured source",
                     REVISION, EDITOR_REVISION, "captured synthetic evidence binding"),
}
ZERO_ACTIVITY_ORACLE_CASES = {
    "admitted_refused_fake_activity_rejected",
    "admitted_conflicted_fake_activity_rejected",
    "admitted_no_effect_fake_activity_rejected",
    "invalid_later_unit_fake_activity_rejected",
}
DVG_ZERO_ACTIVITY_ROWS = {
    "DVG-03": (
        "only `AuthoritativeApplied` creates Project Activity",
        "a zero-authority Receipt creates no Project Activity",
    ),
    "S1-REQ-003": (
        "`AuthoritativeApplied` produces the authoritative Revision",
        "produce only their typed zero-authority Receipt and no Project Activity",
    ),
    "S1-JRN-001": (
        "one applied author effect/Receipt/Activity or one zero-authority Receipt with no Activity",
    ),
    "S1-EVD-003": ("the zero-authority Receipt/no-Activity relation",),
}
DVG_DISPOSITION_ROWS = {
    "DVG-03": (
        "an admitted zero-authority settlement is `PASS-POS`",
        "`PASS-REFUSAL` only for a pre-Admission refusal",
        "`PASS-HOLD` only for an owner-defined recovery hold",
    ),
    "S1-REQ-003": (
        "`PASS-POS` for applied and admitted zero-authority settlements",
        "`PASS-REFUSAL` only before Admission",
        "`PASS-HOLD` only for recovery",
    ),
    "S1-EVD-003": (
        "`PASS-POS` for applied and admitted zero-authority settlements",
        "`PASS-REFUSAL` only before Admission",
        "`PASS-HOLD` only for recovery",
    ),
}
OUTCOME_QUERY_BOOTSTRAP_REQUIREMENTS = (
    "For an `ApplyAuthorEdit` initial attempt whose acknowledgement is wholly absent",
    "`getApplyAuthorEditOutcome` is the first reconciliation action",
    "the original `idempotency_key` in the route and the original challenge nonce only in the "
    "`X-StoryOS-Anti-Forgery` header",
    "does not perform `ExactTransportReplay`, obtain a new challenge, or send the command again "
    "before this read",
)
ACK_LOSS_PROFILE_START = "<!-- ACK_LOSS_AUTHOR_COMMAND_PROFILE_START -->"
ACK_LOSS_PROFILE_END = "<!-- ACK_LOSS_AUTHOR_COMMAND_PROFILE_END -->"
ACK_LOSS_GATE_PROFILE = {
    "profile_id": "storyos.dvg.apply-author-edit-ack-loss.v1",
    "selection": {
        "gates": ["DVG-01", "DVG-02", "DVG-03", "DVG-08", "DVG-11"],
        "evidence_classes": [
            "EC-01", "EC-02", "EC-03", "EC-04", "EC-05",
            "EV-CP", "EV-IT", "EV-INT", "EV-SE",
        ],
        "fixtures": ["FX-CONTRACT-R1", "FX-JOURNAL-GROUP", "FX-SCOPE-2U2P"],
        "fault_points": [
            "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
            "CFP-ADMISSION-EXPIRY",
            "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
            "CFP-CORE-BEFORE-COMMIT",
            "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
            "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
            "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
            "CFP-SCOPE-BEFORE-QUERY",
        ],
        "schedules": ["SCH-NORMAL", "SCH-REORDER", "SCH-SCOPE", "SCH-UNKNOWN"],
        "oracles": [
            "ORC-ATOMIC-AUTHORITY", "ORC-CONTRACT", "ORC-EDITOR-JOURNAL",
            "ORC-NEGATIVE-CLOSURE", "ORC-OUTCOME-UNKNOWN", "ORC-SCOPE",
        ],
        "bundles": ["B-CONTRACT", "B-CORE", "B-EDITOR", "B-SCOPE"],
    },
    "external_dispatch_branch": {
        "owners": ["OWN-PROTO", "OWN-CTX", "OWN-PG", "OWN-RET"],
        "fixtures": [
            "FX-CONTEXT-DISCLOSURE", "FX-FAKE-MODEL", "FX-REAL-MODEL-ADVISORY",
        ],
        "fault_points": [
            "CFP-DISPATCH-AFTER-CLAIM-BEFORE-IO",
            "CFP-DISPATCH-AFTER-IO-BEFORE-CONFIRMATION",
            "CFP-RECONCILIATION-BEFORE-SETTLEMENT",
            "CFP-LATE-RESULT",
        ],
        "schedules": ["SCH-UNKNOWN", "SCH-FENCE"],
        "oracle": "ORC-OUTCOME-UNKNOWN",
        "bundles": ["B-CONTEXT", "B-FAKE"],
        "unknown_disposition": "PASS-UNKNOWN",
        "reconciled_disposition": "PASS-POS",
        "unresolved_disposition": "PASS-HOLD",
        "reconciliation": "separately_admitted_reconciliation_or_new_attempt",
        "fence_and_late_result": "preserved",
        "author_command_outcome_read": False,
    },
    "author_command_branch": {
        "owners": ["OWN-WEB", "OWN-ADM", "OWN-CORE", "OWN-PROTO", "OWN-PG"],
        "operations": [
            "createProjectCommandChallenge", "applyAuthorEdit",
            "getApplyAuthorEditOutcome",
        ],
        "first_reconciliation_action": "getApplyAuthorEditOutcome",
        "repeat": "protected_outcome_get_only",
        "post_replay": "forbidden",
        "new_challenge": "forbidden",
        "new_admission": "forbidden",
        "separately_admitted_reconciliation": "forbidden",
        "process_termination_or_restart": "forbidden",
        "dispositions": {
            "committed": "PASS-POS",
            "rejected": "PASS-REFUSAL",
            "still_unknown_challenge_issued": "PASS-HOLD",
            "still_unknown_admission_committed": "PASS-HOLD",
            "query_transport_unavailable_or_malformed": "PASS-HOLD",
            "canonical_security_or_input_problem_gate": "PASS-REFUSAL",
            "canonical_security_or_input_problem_journal": "QueryUnavailable",
        },
        "journal": {
            "unresolved": "OutcomeQueryUnresolved",
            "dependent_submission": "blocked",
            "payload_and_capsule": "retained",
            "invented_success_or_rejection": "forbidden",
        },
        "authority": {
            "authoritative_applied": "one_receipt_one_activity_one_authority_effect",
            "no_effect_conflicted_refused": "one_receipt_zero_activity_zero_authority",
            "rejected": "zero_admission_receipt_activity_core_authority",
            "post_and_get": "one_settlement_same_command_admission_receipt",
        },
        "security": {
            "bindings": [
                "idempotency_key_path", "nonce_header", "Host", "ProjectScope",
                "current_client_session", "full_stored_binding", "forced_RLS",
            ],
            "route_policy": "SensitiveSafeReadWithRefererFallback",
            "nonce": "header_only_not_url_log_or_response",
            "cache_control": "no-store_on_success_and_problem",
            "failure": "uniform_non_oracular",
            "read_effects": "zero_nonce_consumption_core_receipt_activity_authority",
        },
    },
    "stage1_handoff": {
        "requirements": ["S1-REQ-004:acknowledgement_loss", "S1-EVD-004:acknowledgement_loss"],
        "operations": [
            "createProjectCommandChallenge", "applyAuthorEdit",
            "getApplyAuthorEditOutcome",
        ],
        "gates": ["DVG-01", "DVG-02", "DVG-03", "DVG-08", "DVG-11"],
        "evidence_classes": [
            "EC-01", "EC-02", "EC-03", "EC-04", "EC-05",
            "EV-CP", "EV-IT", "EV-INT", "EV-SE",
        ],
        "fixtures": ["FX-CONTRACT-R1", "FX-JOURNAL-GROUP", "FX-SCOPE-2U2P"],
        "fault_points": [
            "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
            "CFP-ADMISSION-EXPIRY",
            "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
            "CFP-CORE-BEFORE-COMMIT",
            "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
            "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
            "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
            "CFP-SCOPE-BEFORE-QUERY",
        ],
        "schedules": ["SCH-NORMAL", "SCH-REORDER", "SCH-SCOPE", "SCH-UNKNOWN"],
        "oracles": [
            "ORC-ATOMIC-AUTHORITY", "ORC-CONTRACT", "ORC-EDITOR-JOURNAL",
            "ORC-NEGATIVE-CLOSURE", "ORC-OUTCOME-UNKNOWN", "ORC-SCOPE",
        ],
        "bundles": ["B-CONTRACT", "B-CORE", "B-EDITOR", "B-SCOPE"],
    },
    "excluded": {
        "identifiers": [
            "DVG-07", "FX-RECOVERY-EDITOR", "SCH-CRASH",
            "ORC-RECOVERY-ATOMICITY", "B-RECOVERY",
        ],
        "behaviors": [
            "reload", "restart", "takeover", "replay_generation", "snapshot_resync",
            "late_result", "retention", "proposal",
        ],
    },
}
ACK_LOSS_ROW_SHA256 = {
    "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL":
        "1b9647400c92a5bba11fe3e7f00f5571426c271ffaa6b7bfa527a46e61c2593a",
    "SCH-UNKNOWN": "3c762c82fb2c46527211e9f4477684034d85c3a25549a96cd4667d8ba66cc2a1",
    "DVG-08": "1fd599932d12e0fabf57440215e14b21baf7841e09ea9eb13e6b6788c3e66579",
    "ORC-OUTCOME-UNKNOWN":
        "2cf2ec76df8c930a6764695c6b8ea055107f576b1d6c8203c2cd51bddac2ce97",
}


def strict_json_object(pairs: list[tuple[str, object]]) -> dict:
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()
def current_sources() -> tuple[dict, dict, list[dict], dict, dict[str, str]]:
    return (
        json.loads(POLICY_PATH.read_text(encoding="utf-8")),
        json.loads(EVIDENCE_PATH.read_text(encoding="utf-8")),
        [json.loads(line) for line in CANDIDATES_PATH.read_text(encoding="utf-8").splitlines()],
        json.loads(APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_PATH.read_text(encoding="utf-8")),
        {path.as_posix(): path.read_text(encoding="utf-8") for path in PROJECTIONS},
    )


def current_outcome_sources() -> tuple[dict, dict, dict, str, str]:
    return (
        json.loads(APPLY_AUTHOR_EDIT_OUTCOME_SCHEMA_PATH.read_text(encoding="utf-8")),
        json.loads(APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_PATH.read_text(encoding="utf-8")),
        json.loads(ROUTE_CATALOG_PATH.read_text(encoding="utf-8")),
        TYPESCRIPT_CLIENT_PATH.read_text(encoding="utf-8"),
        PROJECTIONS[3].read_text(encoding="utf-8"),
    )
def visible_markdown(text: str) -> str:
    return re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
def union_variant_body(text: str, variant: str, next_variant: str) -> str | None:
    match = re.search(
        rf"\|\s*{variant}\s*\{{(?P<body>.*?)(?=\n\s*\|\s*{next_variant})",
        text,
        flags=re.DOTALL,
    )
    return match.group("body") if match else None
def union_variant_names(text: str) -> tuple[str, ...]:
    return tuple(re.findall(r"^  (?:\| )?([A-Z][A-Za-z0-9]*)\b", text, flags=re.MULTILINE))
def require_union_variant_shape(
    errors: list[str],
    union: str,
    variant: str,
    next_variant: str,
    required: tuple[str, ...],
    forbidden: tuple[str, ...] = (),
) -> None:
    body = union_variant_body(union, variant, next_variant)
    if (body is None or any(value not in body for value in required)
            or any(value in body for value in forbidden)):
        errors.append(f"Web {variant} shape drifted")
def dvg_projection_errors(dvg_projection: str) -> list[str]:
    errors: list[str] = []
    visible_dvg = visible_markdown(dvg_projection)
    for label, requirements_by_row in (
        ("zero-Activity semantics", DVG_ZERO_ACTIVITY_ROWS),
        ("disposition", DVG_DISPOSITION_ROWS),
    ):
        for row_id, required_values in requirements_by_row.items():
            rows = re.findall(
                rf"^\|\s*`{re.escape(row_id)}`.*$",
                visible_dvg,
                flags=re.MULTILINE,
            )
            if len(rows) != 1 or any(value not in rows[0] for value in required_values):
                errors.append(
                    f"{PROJECTIONS[4].name}: {row_id} {label} drifted"
                )
    return errors


def dvg_ack_loss_errors(dvg_projection: str) -> list[str]:
    errors: list[str] = []
    visible_dvg = visible_markdown(dvg_projection)
    if (dvg_projection.count(ACK_LOSS_PROFILE_START) != 1
            or dvg_projection.count(ACK_LOSS_PROFILE_END) != 1):
        return ["DVG acknowledgement-loss profile marker count drifted"]
    profile_match = re.search(
        rf"{re.escape(ACK_LOSS_PROFILE_START)}\s*```json\s*(.*?)\s*```\s*"
        rf"{re.escape(ACK_LOSS_PROFILE_END)}",
        dvg_projection,
        flags=re.DOTALL,
    )
    if not profile_match:
        return ["DVG acknowledgement-loss profile is missing"]
    try:
        profile = json.loads(profile_match.group(1), object_pairs_hook=strict_json_object)
    except (json.JSONDecodeError, ValueError):
        return ["DVG acknowledgement-loss profile is not valid JSON"]
    if profile != ACK_LOSS_GATE_PROFILE:
        errors.append("DVG acknowledgement-loss profile drifted")
    row_requirements = {
        "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL": (
            "valid `getApplyAuthorEditOutcome` response exists only in memory",
            "complete Journal query observation has not committed",
            "repeat only the protected outcome GET",
        ),
        "SCH-UNKNOWN": (
            "External dispatch",
            "Author Command acknowledgement loss",
            "getApplyAuthorEditOutcome",
            "no POST replay, process termination, or restart",
        ),
        "DVG-08": (
            "External-dispatch branch",
            "Author Command branch",
            "getApplyAuthorEditOutcome",
            "`OWN-WEB` + `OWN-ADM` + `OWN-CORE`",
            "`PASS-HOLD`",
        ),
        "ORC-OUTCOME-UNKNOWN": (
            "External post-claim unknown",
            "Author Command acknowledgement loss",
            "`Committed` is `PASS-POS`",
            "`Rejected` is `PASS-REFUSAL`",
            "`StillUnknown` and Query failure are `PASS-HOLD`",
        ),
    }
    for row_id, requirements in row_requirements.items():
        rows = re.findall(
            rf"^\|\s*`{re.escape(row_id)}`.*$", visible_dvg, flags=re.MULTILINE
        )
        if (len(rows) != 1
                or any(requirement not in rows[0] for requirement in requirements)
                or hashlib.sha256(rows[0].encode()).hexdigest()
                != ACK_LOSS_ROW_SHA256[row_id]):
            errors.append(f"DVG {row_id} acknowledgement-loss row drifted")
    return errors
def apply_author_edit_web_errors(schema: dict, web_projection: str) -> list[str]:
    errors: list[str] = []
    effect_variants = schema.get("$defs", {}).get("ApplyAuthorEditEffect", {}).get("oneOf", [])
    variants = {}
    for variant in effect_variants:
        kind = variant.get("properties", {}).get("kind", {}).get("const")
        if isinstance(kind, str):
            variants[kind] = variant
    if len(effect_variants) != len(variants):
        errors.append("applyAuthorEdit effect branches are not unique tagged objects")
    expected_fields = {
        "authoritative_applied": {
            "kind", "authoritative_revision", "authoritative_commit_id",
            "author_action_sequence", "project_activity_position"},
        "no_effect": {"kind", "reason"},
        "conflicted": {"kind", "reason", "current_authoritative_revision_id"},
        "refused": {"kind", "reason"},
    }
    if set(variants) != set(expected_fields):
        errors.append("applyAuthorEdit response effect variants drifted")
    for kind, expected_variant_fields in expected_fields.items():
        variant = variants.get(kind, {})
        properties = variant.get("properties", {})
        required = variant.get("required", [])
        if set(properties) != expected_variant_fields or set(required) != expected_variant_fields:
            errors.append(f"applyAuthorEdit {kind} effect shape drifted")
        if variant.get("additionalProperties") is not False:
            errors.append(f"applyAuthorEdit {kind} no-extra-field guard drifted")
    visible_web = visible_markdown(web_projection)
    group_settlement = re.search(
        r"GroupSettlement\s*=.*?(?=\n\nAuthorSurfaceConvergence\s*=)",
        visible_web,
        flags=re.DOTALL,
    )
    if not group_settlement:
        errors.append("Web GroupSettlement union is missing")
    else:
        settlement = group_settlement.group(0)
        if union_variant_names(settlement) != (
            "Unsettled", "PreAdmissionRefused", "OutcomeQueryRejectedNoAdmission",
            "AppliedReceiptSettled", "ZeroAuthorityReceiptSettled",
            "OtherEditorReceiptSettled", "RequiresReconfirmation"):
            errors.append("Web GroupSettlement variants drifted")
        require_union_variant_shape(
            errors, settlement, "OutcomeQueryRejectedNoAdmission", "AppliedReceiptSettled",
            ("outcome_query_observation_id", "reason: challenge_expired_unconsumed", "observed_at"),
            ("receipt", "project_activity_position"),
        )
        require_union_variant_shape(
            errors, settlement, "AppliedReceiptSettled", "ZeroAuthorityReceiptSettled",
            ("receipt_ref: DomainReceiptRef", "project_activity_position"),
        )
        require_union_variant_shape(
            errors, settlement, "ZeroAuthorityReceiptSettled", "OtherEditorReceiptSettled",
            ("receipt_ref: DomainReceiptRef", "result: NoEffect | Conflicted | Refused"),
            ("project_activity_position",),
        )
        require_union_variant_shape(
            errors, settlement, "OtherEditorReceiptSettled", "RequiresReconfirmation",
            ("receipt_ref: ReceiptRef", "project_activity_position"),
        )
    convergence = re.search(
        r"AuthorSurfaceConvergence\s*=.*?(?=\n\nAuthorAttention\s*=)",
        visible_web,
        flags=re.DOTALL,
    )
    if not convergence:
        errors.append("Web AuthorSurfaceConvergence union is missing")
    else:
        convergence_union = convergence.group(0)
        if union_variant_names(convergence_union) != (
            "Pending", "AppliedReceiptConverged", "ZeroAuthorityReceiptVisible",
            "OtherEditorReceiptConverged", "PreAdmissionRefusalConverged",
            "OutcomeQueryRejectedVisible", "ReconfirmationConverged"):
            errors.append("Web AuthorSurfaceConvergence variants drifted")
        require_union_variant_shape(
            errors, convergence_union, "AppliedReceiptConverged", "ZeroAuthorityReceiptVisible",
            ("receipt_ref: DomainReceiptRef", "project_activity_position", "projection_proof:"),
        )
        require_union_variant_shape(
            errors, convergence_union, "ZeroAuthorityReceiptVisible",
            "OtherEditorReceiptConverged",
            ("receipt_ref: DomainReceiptRef", "result: NoEffect | Conflicted | Refused"),
            ("project_activity_position",),
        )
        require_union_variant_shape(
            errors, convergence_union, "OtherEditorReceiptConverged",
            "PreAdmissionRefusalConverged",
            ("receipt_ref: ReceiptRef", "project_activity_position", "projection_proof:"),
        )
        require_union_variant_shape(
            errors, convergence_union, "OutcomeQueryRejectedVisible",
            "ReconfirmationConverged",
            ("outcome_query_observation_id", "local_rejected_surface_id",
             "preserved_local_payload_ref"),
            ("receipt", "project_activity_position"),
        )
    observations = re.search(
        r"BrowserProtocolObservation\s*=.*?(?=\n```)",
        visible_web,
        flags=re.DOTALL,
    )
    if not observations:
        errors.append("Web BrowserProtocolObservation union is missing")
    else:
        observation_union = observations.group(0)
        if union_variant_names(observation_union) != (
            "PreAdmissionProblemObservation", "UnresolvedTransportProblemObservation",
            "AcceptedObservation", "ApplyAuthorEditAppliedObservation",
            "ApplyAuthorEditZeroAuthorityObservation", "OtherEditorCommittedObservation",
            "RequiresReconfirmationObservation", "OutcomeUnknownProblemObservation"):
            errors.append("Web BrowserProtocolObservation variants drifted")
        require_union_variant_shape(
            errors, observation_union, "ApplyAuthorEditAppliedObservation",
            "ApplyAuthorEditZeroAuthorityObservation",
            ("receipt: DomainReceipt { result: AuthoritativeApplied }",
             "effect: AuthoritativeApplied {", "project_activity_position"),
        )
        require_union_variant_shape(
            errors, observation_union, "ApplyAuthorEditZeroAuthorityObservation",
            "OtherEditorCommittedObservation",
            ("receipt: DomainReceipt { result: NoEffect | Conflicted | Refused }",
             "effect: NoEffect | Conflicted | Refused"),
            ("project_activity_position",),
        )
        require_union_variant_shape(
            errors, observation_union, "OtherEditorCommittedObservation",
            "RequiresReconfirmationObservation",
            ("receipt_ref: ReceiptRef", "project_activity_position"),
        )
    return errors


def apply_author_edit_outcome_web_errors(web_projection: str) -> list[str]:
    canonical_web = re.sub(r"\s+", " ", visible_markdown(web_projection))
    if any(requirement not in canonical_web
           for requirement in OUTCOME_QUERY_BOOTSTRAP_REQUIREMENTS):
        return ["Web missing-first-ack outcome Query bootstrap drifted"]
    return []


def apply_author_edit_outcome_contract_errors(
    outcome_schema: dict,
    response_schema: dict,
    route_catalog: dict,
    typescript_client: str,
    web_projection: str,
) -> list[str]:
    errors: list[str] = []
    if hashlib.sha256(json.dumps(
            outcome_schema, sort_keys=True, separators=(",", ":")
    ).encode()).hexdigest() != "62fd88c95fbb813c650a75fa26f5374f1cb970059c6b31a0c105909ee4b283e1":
        errors.append("outcome Query generated schema drifted")
    root_properties = outcome_schema.get("properties", {})
    if (outcome_schema.get("$id") != "storyos.query.apply-author-edit-outcome.response.v1"
            or outcome_schema.get("type") != "object"
            or set(root_properties) != {"schema_id", "correlation_id", "project_scope", "outcome"}
            or set(outcome_schema.get("required", [])) != set(root_properties)
            or outcome_schema.get("additionalProperties") is not False
            or root_properties.get("outcome") != {"$ref": "#/$defs/ApplyAuthorEditOutcome"}
            or root_properties.get("project_scope") != {"$ref": "#/$defs/ProjectScope"}
            or root_properties.get("schema_id") != {"type": "string"}
            or root_properties.get("correlation_id") != {"type": "string"}):
        errors.append("outcome Query response envelope drifted")
    definitions = outcome_schema.get("$defs", {})
    outcome_variants = definitions.get("ApplyAuthorEditOutcome", {}).get("oneOf", [])
    tagged_outcomes = {
        variant.get("properties", {}).get("outcome_kind", {}).get("const"): variant
        for variant in outcome_variants
    }
    expected_outcomes = {
        "committed": {"outcome_kind", "response"},
        "rejected": {"outcome_kind", "reason"},
        "still_unknown": {"outcome_kind", "observation"},
    }
    if set(tagged_outcomes) != set(expected_outcomes) or len(tagged_outcomes) != len(outcome_variants):
        errors.append("outcome Query public branches drifted")
    for kind, expected_fields in expected_outcomes.items():
        variant = tagged_outcomes.get(kind, {})
        if (variant.get("type") != "object"
                or variant.get("properties", {}).get("outcome_kind")
                != {"const": kind, "type": "string"}
                or set(variant.get("properties", {})) != expected_fields
                or set(variant.get("required", [])) != expected_fields
                or variant.get("additionalProperties") is not False):
            errors.append(f"outcome Query {kind} branch drifted")
    for kind, field, target in (
        ("committed", "response", "ApplyAuthorEditResponse"),
        ("rejected", "reason", "ApplyAuthorEditRejectionReason"),
        ("still_unknown", "observation", "ApplyAuthorEditUnknownObservation"),
    ):
        if tagged_outcomes.get(kind, {}).get("properties", {}).get(field) != {
            "$ref": f"#/$defs/{target}"
        }:
            errors.append(f"outcome Query {kind} payload binding drifted")
    unknown_variants = definitions.get("ApplyAuthorEditUnknownObservation", {}).get("oneOf", [])
    tagged_unknowns = {
        variant.get("properties", {}).get("observation_kind", {}).get("const"): variant
        for variant in unknown_variants
    }
    expected_unknowns = {
        "challenge_issued": {"observation_kind", "expires_at"},
        "admission_committed": {
            "observation_kind", "command_id", "author_command_admission_id",
            "reconciliation_required",
        },
    }
    if set(tagged_unknowns) != set(expected_unknowns) or len(tagged_unknowns) != len(unknown_variants):
        errors.append("outcome Query unknown observation branches drifted")
    for kind, expected_fields in expected_unknowns.items():
        variant = tagged_unknowns.get(kind, {})
        if (variant.get("type") != "object"
                or variant.get("properties", {}).get("observation_kind")
                != {"const": kind, "type": "string"}
                or set(variant.get("properties", {})) != expected_fields
                or set(variant.get("required", [])) != expected_fields
                or variant.get("additionalProperties") is not False):
            errors.append(f"outcome Query {kind} observation drifted")
    marker = tagged_unknowns.get("admission_committed", {}).get(
        "properties", {}
    ).get("reconciliation_required", {})
    if marker != {"const": True, "type": "boolean"}:
        errors.append("outcome Query reconciliation marker drifted")
    if (tagged_unknowns.get("challenge_issued", {}).get("properties", {}).get("expires_at")
            != {"format": "date-time", "type": "string"}
            or any(tagged_unknowns.get("admission_committed", {}).get(
                "properties", {}
            ).get(field) != {"type": "string"}
                for field in ("command_id", "author_command_admission_id"))):
        errors.append("outcome Query unknown identity field drifted")
    if definitions.get("ApplyAuthorEditRejectionReason") != {
        "enum": ["challenge_expired_unconsumed"], "type": "string"
    }:
        errors.append("outcome Query rejection reason drifted")
    response_root = {
        key: response_schema.get(key)
        for key in ("type", "additionalProperties", "properties", "required")
    }
    if (response_schema.get("$id") != "storyos.command.apply-author-edit.response.v2"
            or definitions.get("ApplyAuthorEditResponse") != response_root
            or any(definitions.get(name) != definition
                   for name, definition in response_schema.get("$defs", {}).items())):
        errors.append("outcome Query committed response-v2 binding drifted")

    routes = [
        operation for operation in route_catalog.get("operations", [])
        if operation.get("operation_id") == "getApplyAuthorEditOutcome"
    ]
    if len(routes) != 1:
        errors.append("outcome Query route identity drifted")
    elif hashlib.sha256(json.dumps(
            routes[0], sort_keys=True, separators=(",", ":")
    ).encode()).hexdigest() != "683b155b07142e25ffe370fa1b10e533351b5aa42bcba88315afd623d33f3878":
        errors.append("outcome Query route proof drifted")
    for alias, expected_client_shape in (
        ("ApplyAuthorEditRejectionReason",
         'export type ApplyAuthorEditRejectionReason = "challenge_expired_unconsumed";'),
        ("ApplyAuthorEditUnknownObservation",
         'export type ApplyAuthorEditUnknownObservation = { "observation_kind": '
         '"challenge_issued", expires_at: string, } | { "observation_kind": '
         '"admission_committed", command_id: string, author_command_admission_id: string, '
         'reconciliation_required: true, };'),
        ("ApplyAuthorEditOutcome",
         'export type ApplyAuthorEditOutcome = { "outcome_kind": "committed", response: '
         'ApplyAuthorEditResponse, } | { "outcome_kind": "rejected", reason: '
         'ApplyAuthorEditRejectionReason, } | { "outcome_kind": "still_unknown", observation: '
         'ApplyAuthorEditUnknownObservation, };'),
        ("GetApplyAuthorEditOutcomeResponse",
         "export type GetApplyAuthorEditOutcomeResponse = { schema_id: string, correlation_id: "
         "string, project_scope: ProjectScope, outcome: ApplyAuthorEditOutcome, };")
    ):
        if re.findall(rf"^export type {alias} = .*;$", typescript_client, re.MULTILINE) != [
            expected_client_shape
        ]:
            errors.append("outcome Query generated TypeScript drifted")
            break
    expected_method = (
        "export declare function getApplyAuthorEditOutcome(options: StoryOSQueryOptions & { "
        "projectId: string; idempotencyKey: string; antiForgery: string }): "
        "Promise<GetApplyAuthorEditOutcomeResponse>;"
    )
    if re.findall(r"^export declare function getApplyAuthorEditOutcome.*;$",
                  typescript_client, re.MULTILINE) != [expected_method]:
        errors.append("outcome Query generated TypeScript drifted")

    visible_web = visible_markdown(web_projection)
    for marker in (
        "GroupReconciliation =", "GroupSettlement =", "AuthorSurfaceConvergence =",
        "ApplyAuthorEditOutcomeQueryAttempt =", "ApplyAuthorEditOutcomeQueryObservation =",
        "OutcomeQueryReducer =",
    ):
        if len(re.findall(rf"^{re.escape(marker)}", visible_web, re.MULTILINE)) != 1:
            errors.append("Web outcome Query definition count drifted")
    reconciliation = re.search(
        r"GroupReconciliation\s*=.*?(?=\n\nGroupSettlement\s*=)",
        visible_web,
        flags=re.DOTALL,
    )
    if not reconciliation or union_variant_names(reconciliation.group(0)) != (
        "NoReconciliationNeeded", "TransportOrAdmissionUnknown", "KnownSettlementQuery",
        "ProtocolIncompatibleAccepted", "OutcomeQueryUnresolved", "ProvenNoAdmission",
        "TerminalResolved",
    ):
        errors.append("Web GroupReconciliation outcome Query variants drifted")
    elif hashlib.sha256((union_variant_body(
            reconciliation.group(0), "OutcomeQueryUnresolved", "ProvenNoAdmission"
    ) or "").encode()).hexdigest() != (
        "80ddb41a2b9fe2aa9af56ba39abdd4dee274b59fd91d9511c441d5fcda1fb727"
    ):
        errors.append("Web OutcomeQueryUnresolved reconciliation shape drifted")
    terminal = re.search(
        r"\| TerminalResolved \{(?P<body>.*)\n    \}\s*$",
        reconciliation.group(0) if reconciliation else "", re.DOTALL,
    )
    if (not terminal or hashlib.sha256(terminal.group("body").encode()).hexdigest()
            != "5b76791464cca14c555916e9341a7dd6c45ffc71d14c01cde75c0cfc185d2b19"):
        errors.append("Web outcome Query terminal relation drifted")
    for label, pattern, variant, next_variant, expected_hash in (
        ("settlement", r"GroupSettlement\s*=.*?(?=\n\nAuthorSurfaceConvergence\s*=)",
         "OutcomeQueryRejectedNoAdmission", "AppliedReceiptSettled",
         "a51546196f32b918f30f84592004fb3ad4ff3d990f841332df85a222d8c2c781"),
        ("visible", r"AuthorSurfaceConvergence\s*=.*?(?=\n\nAuthorAttention\s*=)",
         "OutcomeQueryRejectedVisible", "ReconfirmationConverged",
         "e89fbcd0aedbcf9720f2542dadb6a1639d0bb0ae3b9657b8f6163c88938c77f9"),
    ):
        union = re.search(pattern, visible_web, re.DOTALL)
        body = union_variant_body(union.group(0), variant, next_variant) if union else ""
        if hashlib.sha256((body or "").encode()).hexdigest() != expected_hash:
            errors.append(f"Web outcome Query rejected {label} relation drifted")
    for label, start, end, expected_hash in (
        ("attempt", "ApplyAuthorEditOutcomeQueryAttempt =",
         "ApplyAuthorEditOutcomeQueryObservation =",
         "e02c80e55ce08c985c2af6a461787c0d13a59fe9cf3dec706b598a72367d7773"),
        ("observation", "ApplyAuthorEditOutcomeQueryObservation =", "OutcomeQueryReducer =",
         "191508ba651a7c632a67385193afb5dbffdfaafb8bcc21f6b00c5412a6cee22f"),
    ):
        block = re.search(
            rf"{re.escape(start)}.*?(?=\n\n{re.escape(end)})",
            visible_web,
            flags=re.DOTALL,
        )
        if (not block
                or hashlib.sha256(block.group(0).encode()).hexdigest() != expected_hash):
            errors.append(f"Web outcome Query {label} shape drifted")
    reducer = re.search(r"OutcomeQueryReducer =.*?(?=\n```)", visible_web, flags=re.DOTALL)
    expected_reducer = """OutcomeQueryReducer =
  NoOutcomeObserved -> ChallengeIssued | AdmissionCommitted | Rejected | Committed
  ChallengeIssued(exact same expires_at) -> ChallengeIssued | AdmissionCommitted | Rejected | Committed
  AdmissionCommitted(same Command and Admission) -> AdmissionCommitted | Committed(same identities)
  QueryUnavailable -> preserve strongest_valid_observation
  Rejected | Committed -> terminal immutable
  late original POST acknowledgement + GET Committed -> one exact settlement"""
    if not reducer or reducer.group(0) != expected_reducer:
        errors.append("Web outcome Query reducer variants drifted")
    canonical_web = re.sub(r"\s+", " ", visible_web)
    for required_meaning in (
        "OutcomeQueryRejectedNoAdmission { outcome_query_observation_id",
        "OutcomeQueryRejectedVisible { outcome_query_observation_id",
        "All three records name the same exact query observation",
        "The safe outcome Query does not consume the nonce, append Server lifecycle state, "
        "invoke Core, create a Receipt or Activity, or change authority",
        "A query transport failure, malformed response, or canonical non-200 Problem remains "
        "unresolved",
        "No `Rejected` or `StillUnknown` branch releases the dependent queue, collects the "
        "capsule or payload, invokes or replays the command, or silently succeeds",
        "The editor does not display saved, rejected, or settled and does not release a "
        "dependent group until that complete Journal transaction commits",
        "A changed Project Scope, idempotency key, Command, Admission, Receipt, command "
        "correlation, digest, or result relation fails closed",
        "This client-first bootstrap applies only to `ApplyAuthorEdit`",
        "After reload, crash, process restart, or continued current-process recovery, an "
        "`ApplyAuthorEdit` `DeliveryUnknown` state with an available exact capsule and "
        "still-valid Client Session permits only the protected outcome Query before any "
        "command replay",
        "Section 10 consumes this same rule; it does not select `ExactTransportReplay` "
        "for `ApplyAuthorEdit`",
        "The read uses `SensitiveSafeReadWithRefererFallback`. Every success and Problem "
        "response is `Cache-Control: no-store`; a cache is never outcome evidence",
    ):
        if required_meaning not in canonical_web:
            errors.append("Web outcome Query closed meaning drifted")
            break
    errors.extend(web_reload_get_first_errors(canonical_web))
    return errors


def web_reload_get_first_errors(canonical_web: str) -> list[str]:
    errors: list[str] = []
    if (
        "| send may have occurred, crash before response observation commits | "
        "reopen the exact capsule and append only `ExactTransportReplay` through the "
        "original command route |"
    ) in canonical_web:
        errors.append("Web reload matrix still requires ExactTransportReplay for ApplyAuthorEdit")
    if (
        "Without a reload, an `ApplyAuthorEdit` `DeliveryUnknown` state permits only the "
        "protected outcome Query before any command replay"
    ) in canonical_web:
        errors.append("Web reload GET-first meaning drifted")
    for required_meaning in (
        "send may have occurred, crash before response observation commits, "
        "`ApplyAuthorEdit`, capsule available, Client Session binding still exact",
        "call `getApplyAuthorEditOutcome` first with the stored key and nonce",
        "send may have occurred, crash before response observation commits, another "
        "editor command, exact capsule and Client Session still valid | reopen the exact "
        "capsule and append only `ExactTransportReplay`",
        "`ApplyAuthorEdit` delivery unknown and Client Session binding is invalid/expired "
        "or capsule/request equality is unverifiable | remain "
        "`OutcomeQueryUnresolved { NoOutcomeObserved }`",
        "Query before any command replay, including after reload, crash, or process "
        "restart",
        "for missing `ApplyAuthorEdit` acknowledgement use the protected outcome Query; "
        "otherwise use an actually supplied `outcome_unknown` query",
        "the `ApplyAuthorEdit` GET-first path from delivery unknown to committed, "
        "rejected, or inspectable unknown, including after reload, crash, or process restart",
        "Server or PostgreSQL process interruption does not create a different first "
        "browser action",
        "The browser follows the same DeliveryUnknown or durable-observation matrix",
        "Server or PostgreSQL process interruption uses this same section 10 matrix",
        "frozen/unsubmitted journal group, pre-admission refusal, or "
        "delivery/Admission uncertainty | local journal/capsule only; no Host Artifact "
        "ingress and no `RecoveryDraft` claim",
        "takeover delivery unknown | exact transport replay is allowed only with its "
        "available capsule and current binding",
        "Event arrival alone does not settle the local group",
    ):
        if required_meaning not in canonical_web:
            errors.append("Web reload GET-first meaning drifted")
            break
    return errors


def author_admission_errors(admission_projection: str) -> list[str]:
    errors: list[str] = []
    visible_admission = visible_markdown(admission_projection)
    settlement = re.search(
        r"AuthorCommandAdmissionSettlement\s*=.*?(?=\n```)",
        visible_admission,
        flags=re.DOTALL,
    )
    if not settlement:
        errors.append("Admission settlement union is missing")
    else:
        receipt_settled = re.search(
            r"ReceiptSettled\s*\{(?P<body>.*?)(?=\n\s*\}\n\s*\|\s*RequiresReconfirmation)",
            settlement.group(0),
            flags=re.DOTALL,
        )
        body = receipt_settled.group("body") if receipt_settled else ""
        canonical_body = re.sub(r"\s+", " ", body).strip()
        expected_body = (
            "receipt_ref, activity_relation: ActivityBacked "
            "{ project_activity_position } | ReceiptOnly, settled_at"
        )
        if not receipt_settled or canonical_body != expected_body:
            errors.append("Admission ReceiptSettled branch shape drifted")
    applied_row = re.findall(
        r"^\|\s*`AuthoritativeApplied`\s*\|.*$",
        visible_admission,
        flags=re.MULTILINE,
    )
    zero_row = re.findall(
        r"^\|\s*`NoEffect`, `Conflicted`, or `Refused`\s*\|.*$",
        visible_admission,
        flags=re.MULTILINE,
    )
    if (len(applied_row) != 1
            or "`ActivityBacked { project_activity_position }`" not in applied_row[0]
            or "exactly one" not in applied_row[0]):
        errors.append("Admission AuthoritativeApplied Activity mapping drifted")
    if (len(zero_row) != 1 or "`ReceiptOnly`" not in zero_row[0]
            or "one typed Receipt" not in zero_row[0]
            or "zero Project Activity" not in zero_row[0]):
        errors.append("Admission zero-authority Receipt-only mapping drifted")
    for requirement in (
        "one-to-one linkage among Admission, Command, and typed Receipt",
        "one-to-one linkage between each `ActivityBacked` typed Receipt and its Project Activity",
        "zero Project Activity for each `ReceiptOnly` settlement",
    ):
        if requirement not in visible_admission:
            errors.append(f"Admission cardinality lost {requirement}")
    stale_cardinality = (
        "one-to-one linkage among admission, command,\n"
        "  Receipt, and Project Activity"
    )
    if stale_cardinality in visible_admission:
        errors.append("Admission retains mandatory four-way Activity cardinality")
    return errors


def policy_errors(
    policy: dict,
    evidence: dict,
    candidate_metrics: list[dict],
    apply_author_edit_response_schema: dict,
    projections: dict[str, str],
) -> list[str]:
    errors: list[str] = []
    selected = policy.get("selected", {})
    candidates = policy.get("candidates", {})
    identity = policy.get("identity_binding", {})
    receipt = policy.get("receipt_boundary", {})
    calibration = policy.get("future_anonymous_calibration", {})
    expected_selected = {
        "adjacent_completed_intent_idle_ms": 250,
        "max_author_edit_units": 240,
        "max_normalized_primitives": 240,
        "max_public_json_command_body_bytes": 1048576,
        "limit_profile_revision": "storyos.foundation.absolute.v1",
    }
    if policy.get("schema") != "storyos.author-edit-batch-policy.v1":
        errors.append("policy schema drifted")
    if policy.get("revision") != REVISION or policy.get("status") != "conservative_prerelease":
        errors.append("policy identity or prerelease status drifted")
    if policy.get("owner", {}).get("policy_issue") != "https://github.com/FrankQDWang/StoryOS/issues/70":
        errors.append("#70 is not the policy owner")
    if selected != expected_selected:
        errors.append("selected prerelease ceilings drifted")
    if candidates != {
        "adjacent_completed_intent_idle_ms": [250, 500, 750],
        "max_author_edit_units": [64, 128, 240, 256],
    }:
        errors.append("candidate sweep drifted")
    if identity != {
        "editor_contract_revision": EDITOR_REVISION,
        "request_digest_field": "editor_contract_revision",
        "wire_change": False,
        "core_local_coverage_claim": False,
    }:
        errors.append("editor-contract mapping or no-wire/no-Core-coverage boundary drifted")
    if receipt != {
        "receipt_free": [
            "pre_admission_refusal",
            "infrastructure_or_transaction_failure_before_commit",
        ],
        "typed_zero_authority_receipt_after_core_starts": [
            "refused", "conflicted", "no_effect"
        ],
    }:
        errors.append("Receipt boundary drifted")
    if policy.get("evidence", {}).get("uses_real_user_content") is not False:
        errors.append("policy evidence must prohibit real user content")
    if calibration.get("minimum_opted_in_sessions") != 30 \
            or calibration.get("minimum_completed_intent_counters") != 100000 \
            or calibration.get("allowed_data") != "aggregated counters and histograms only":
        errors.append("future anonymous calibration gate drifted")
    if evidence.get("policy_revision") != REVISION or evidence.get("policy_sha256") != sha256(POLICY_PATH):
        errors.append("captured evidence is not bound to the exact policy source")
    if evidence.get("synthetic_only") is not True or evidence.get("real_user_content") is not False:
        errors.append("captured evidence is not synthetic-only")
    if evidence.get("candidate_order") != {
        "idle_ms": candidates.get("adjacent_completed_intent_idle_ms"),
        "max_units": candidates.get("max_author_edit_units"),
    }:
        errors.append("captured candidate order drifted from policy")
    expected_pairs = {(idle, units) for idle in candidates.get("adjacent_completed_intent_idle_ms", [])
                      for units in candidates.get("max_author_edit_units", [])}
    actual_pairs = {(item.get("idle_ms"), item.get("max_units")) for item in candidate_metrics}
    required_scenarios = {"continuous_english", "slow_english", "boundary_english",
                          "chinese_ime_confirm_cancel", "paste_delete_replace",
                          "hard_boundaries", "pressure"}
    required_metrics = {"request_count", "max_units_per_request", "max_payload_bytes",
                        "journal_storage_bytes", "submission_group_storage_bytes",
                        "grouping_evaluation_latency_ms", "browser_input_latency_ms"}
    if actual_pairs != expected_pairs or any(set(item.get("scenarios", {})) != required_scenarios
            or any(set(metrics) != required_metrics
                   or not isinstance(metrics.get("grouping_evaluation_latency_ms"), (int, float))
                   or metrics.get("grouping_evaluation_latency_ms", -1) < 0
                   or metrics.get("browser_input_latency_ms", {}).get("classification") != "advisory"
                   or any(not isinstance(metrics.get("browser_input_latency_ms", {}).get(key), (int, float))
                          for key in ("max", "p95"))
                   for metrics in item.get("scenarios", {}).values()) for item in candidate_metrics):
        errors.append("captured per-candidate metric matrix is incomplete")
    if evidence.get("candidate_metric_records") != policy.get("evidence", {}).get("captured_candidates"):
        errors.append("candidate metric evidence path drifted")
    semantic_oracle = evidence.get("semantic_oracle", {})
    if not semantic_oracle or not all(semantic_oracle.values()):
        errors.append("captured semantic oracle is incomplete or failed")
    if not ZERO_ACTIVITY_ORACLE_CASES.issubset(semantic_oracle):
        errors.append("captured semantic oracle lost explicit fake-Activity rejection")
    adoption = evidence.get("adoption", {})
    if adoption.get("decision") != "adopt_conservative_prerelease" \
            or adoption.get("idle_ms") != selected.get("adjacent_completed_intent_idle_ms") \
            or adoption.get("max_units") != selected.get("max_author_edit_units"):
        errors.append("captured adoption does not match the selected prerelease policy")
    for path, required_values in PROJECTION_REQUIREMENTS.items():
        projection = visible_markdown(projections.get(path.as_posix(), ""))
        if "storyos.author-edit-batch.release-1.v1" in projection:
            errors.append(f"{path.name} retains the superseded policy identity")
        for required in required_values:
            if required not in projection:
                errors.append(f"{path.name} projection lost {required}")
    errors.extend(dvg_projection_errors(projections.get(PROJECTIONS[4].as_posix(), "")))
    errors.extend(dvg_ack_loss_errors(projections.get(PROJECTIONS[4].as_posix(), "")))
    errors.extend(apply_author_edit_web_errors(
        apply_author_edit_response_schema,
        projections.get(PROJECTIONS[3].as_posix(), ""),
    ))
    errors.extend(apply_author_edit_outcome_web_errors(
        projections.get(PROJECTIONS[3].as_posix(), ""),
    ))
    errors.extend(author_admission_errors(
        projections.get(AUTHOR_ADMISSION_PATH.as_posix(), ""),
    ))
    return errors
def self_test() -> None:
    policy, evidence, candidate_metrics, response_schema, projections = current_sources()
    assert policy_errors(policy, evidence, candidate_metrics, response_schema, projections) == []
    outcome_schema, outcome_response_schema, route_catalog, typescript_client, web_projection = (
        current_outcome_sources()
    )
    assert apply_author_edit_outcome_contract_errors(
        outcome_schema, outcome_response_schema, route_catalog, typescript_client, web_projection
    ) == []
    for path, value in (
        (("selected", "max_author_edit_units"), 999),
        (("identity_binding", "wire_change"), True),
        (("receipt_boundary", "receipt_free"), ["refused"]),
    ):
        changed = deepcopy(policy)
        changed[path[0]][path[1]] = value
        assert policy_errors(changed, evidence, candidate_metrics, response_schema, projections)
    changed_evidence = deepcopy(evidence)
    changed_evidence["semantic_oracle"]["whole_batch_has_one_final_settlement"] = False
    assert policy_errors(policy, changed_evidence, candidate_metrics, response_schema, projections)
    changed_evidence = deepcopy(evidence)
    del changed_evidence["semantic_oracle"]["admitted_no_effect_fake_activity_rejected"]
    assert "lost explicit fake-Activity rejection" in "\n".join(
        policy_errors(policy, changed_evidence, candidate_metrics, response_schema, projections)
    )
    assert policy_errors(policy, evidence, candidate_metrics[:-1], response_schema, projections)
    changed_metrics = deepcopy(candidate_metrics)
    changed_metrics[0]["scenarios"]["pressure"]["grouping_evaluation_latency_ms"] = "fast"
    assert policy_errors(policy, evidence, changed_metrics, response_schema, projections)
    for path, required_values in PROJECTION_REQUIREMENTS.items():
        changed_projections = dict(projections)
        changed_projections[path.as_posix()] = changed_projections[path.as_posix()].replace(
            required_values[0], ""
        )
        assert path.name in "\n".join(
            policy_errors(policy, evidence, candidate_metrics, response_schema, changed_projections)
        )
    changed_projections = dict(projections)
    adr = PROJECTIONS[1].as_posix()
    changed_projections[adr] = changed_projections[adr].replace("240-intent", "999-intent")
    assert PROJECTIONS[1].name in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, response_schema, changed_projections)
    )
    changed_projections[adr] = f"<!--{projections[adr]}-->"
    assert PROJECTIONS[1].name in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, response_schema, changed_projections)
    )
    dvg = PROJECTIONS[4].as_posix()
    for label, requirements_by_row in (
        ("zero-Activity semantics", DVG_ZERO_ACTIVITY_ROWS),
        ("disposition", DVG_DISPOSITION_ROWS),
    ):
        for row_id, required_values in requirements_by_row.items():
            row = re.findall(
                rf"^\|\s*`{re.escape(row_id)}`.*$",
                projections[dvg],
                flags=re.MULTILINE,
            )[0]
            changed_row = row.replace(required_values[0], "drifted", 1)
            changed_projections = dict(projections)
            changed_projections[dvg] = changed_projections[dvg].replace(
                row, changed_row, 1
            )
            assert f"{row_id} {label} drifted" in "\n".join(
                policy_errors(policy, evidence, candidate_metrics, response_schema,
                              changed_projections)
            )
    for row_id in ("S1-REQ-003", "S1-EVD-003"):
        changed_projections = dict(projections)
        changed_projections[dvg] = re.sub(
            rf"(^\| `{row_id}` \|.*?)`PASS-POS`",
            rf"\1`PASS-REFUSAL`",
            changed_projections[dvg],
            count=1,
            flags=re.MULTILINE,
        )
        assert f"{row_id} disposition drifted" in "\n".join(
            policy_errors(policy, evidence, candidate_metrics, response_schema,
                          changed_projections)
        )
    for old, new in (
        ('"gates": ["DVG-01", "DVG-02", "DVG-03", "DVG-08", "DVG-11"]',
         '"gates": ["DVG-08"]'),
        ('"getApplyAuthorEditOutcome"', '"getCommand"'),
        ('"CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",', ""),
        ('"committed": "PASS-POS"', '"committed": "PASS-HOLD"'),
        ('"rejected": "PASS-REFUSAL"', '"rejected": "PASS-POS"'),
        ('"query_transport_unavailable_or_malformed": "PASS-HOLD"',
         '"query_transport_unavailable_or_malformed": "PASS-POS"'),
        ('"post_replay": "forbidden"', '"post_replay": "permitted"'),
        ('"author_command_outcome_read": false',
         '"author_command_outcome_read": true'),
        ('"unknown_disposition": "PASS-UNKNOWN"',
         '"unknown_disposition": "PASS-POS"'),
        ('"fence_and_late_result": "preserved"',
         '"fence_and_late_result": "removed"'),
        ('"CFP-LATE-RESULT"', '"CFP-EDITOR-BEFORE-GROUP-ADMISSION"'),
        ('"FX-REAL-MODEL-ADVISORY"', '"FX-RECOVERY-EDITOR"'),
        ('"applyAuthorEdit",', ""),
        ('"fixtures": ["FX-CONTRACT-R1", "FX-JOURNAL-GROUP", "FX-SCOPE-2U2P"]',
         '"fixtures": ["FX-RECOVERY-EDITOR"]'),
        ('"cache_control": "no-store_on_success_and_problem"',
         '"cache_control": "cacheable"'),
    ):
        changed_projections = dict(projections)
        changed_projections[dvg] = changed_projections[dvg].replace(old, new, 1)
        assert "acknowledgement-loss profile drifted" in "\n".join(
            policy_errors(policy, evidence, candidate_metrics, response_schema,
                          changed_projections)
        )
    changed_projections = dict(projections)
    changed_projections[dvg] = changed_projections[dvg].replace(
        '"post_replay": "forbidden",',
        '"post_replay": "permitted",\n    "post_replay": "forbidden",',
        1,
    )
    assert "profile is not valid JSON" in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, response_schema,
                      changed_projections)
    )
    for row_id, old, new in (
        ("CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
         "repeat only the protected outcome GET", "repeat the POST"),
        ("CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
         "No saved, rejected, settled, queue-release, Receipt, Activity, or authority projection",
         "A saved projection"),
        ("SCH-UNKNOWN", "no POST replay, process termination, or restart",
         "POST replay is allowed"),
        ("SCH-UNKNOWN", "before command commit", "after command commit"),
        ("DVG-08", "Author Command branch", "External-only branch"),
        ("DVG-08", "never creates a reconciliation command or POST Attempt",
         "may create a new challenge and Admission"),
        ("ORC-OUTCOME-UNKNOWN", "`Rejected` is `PASS-REFUSAL`",
         "`Rejected` is `PASS-POS`"),
        ("ORC-OUTCOME-UNKNOWN",
         "A scheduled canonical security or input Problem is gate-level `PASS-REFUSAL` but leaves the Journal unresolved",
         "A canonical Problem terminally rejects the Journal"),
        ("ORC-OUTCOME-UNKNOWN", "No unresolved branch invents success",
         "`StillUnknown` is also `PASS-POS`. No unresolved branch invents success"),
    ):
        changed_projections = dict(projections)
        changed_projections[dvg] = changed_projections[dvg].replace(old, new, 1)
        assert f"{row_id} acknowledgement-loss row drifted" in "\n".join(
            policy_errors(policy, evidence, candidate_metrics, response_schema,
                          changed_projections)
        )
    changed_schema = deepcopy(response_schema)
    zero_variant = next(
        variant for variant in changed_schema["$defs"]["ApplyAuthorEditEffect"]["oneOf"]
        if variant["properties"]["kind"]["const"] == "no_effect"
    )
    zero_variant["properties"]["project_activity_position"] = {"type": "string"}
    zero_variant["required"].append("project_activity_position")
    assert "no_effect effect shape drifted" in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, changed_schema, projections))
    changed_schema = deepcopy(response_schema)
    applied_variant = next(
        variant for variant in changed_schema["$defs"]["ApplyAuthorEditEffect"]["oneOf"]
        if variant["properties"]["kind"]["const"] == "authoritative_applied"
    )
    del applied_variant["properties"]["project_activity_position"]
    applied_variant["required"].remove("project_activity_position")
    assert "authoritative_applied effect shape drifted" in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, changed_schema, projections)
    )
    changed_schema = deepcopy(response_schema)
    effects = changed_schema["$defs"]["ApplyAuthorEditEffect"]["oneOf"]
    effects.append(deepcopy(effects[0]))
    assert "effect branches are not unique tagged objects" in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, changed_schema, projections))
    effects[-1] = {"type": "null"}
    assert "effect branches are not unique tagged objects" in "\n".join(
        policy_errors(policy, evidence, candidate_metrics, changed_schema, projections))
    web = PROJECTIONS[3].as_posix()
    changed_projections = dict(projections)
    changed_projections[web] = changed_projections[web].replace(
        OUTCOME_QUERY_BOOTSTRAP_REQUIREMENTS[0], "drifted", 1
    )
    assert "outcome Query bootstrap drifted" in "\n".join(policy_errors(
        policy, evidence, candidate_metrics, response_schema, changed_projections))
    for old, new, expected_error in (
        ("receipt_ref: DomainReceiptRef\n      result: NoEffect | Conflicted | Refused",
         "receipt_ref: DomainReceiptRef\n      project_activity_position\n"
         "      result: NoEffect | Conflicted | Refused",
         "ZeroAuthorityReceiptSettled shape drifted"),
        ("  | AppliedReceiptConverged {", "  | LostAppliedReceiptConverged {",
         "AppliedReceiptConverged shape drifted"),
        ("      receipt_ref: DomainReceiptRef\n      project_activity_position",
         "      project_activity_position", "AppliedReceiptSettled shape drifted"),
        ("      result: NoEffect | Conflicted | Refused\n      committed_at",
         "      committed_at", "ZeroAuthorityReceiptSettled shape drifted"),
        ("  | ZeroAuthorityReceiptSettled {",
         "  | ReceiptSettled {\n      receipt_ref: ReceiptRef\n"
         "      project_activity_position\n    }\n  | ZeroAuthorityReceiptSettled {",
         "GroupSettlement variants drifted"),
    ):
        changed_projections = dict(projections)
        changed_projections[web] = changed_projections[web].replace(old, new, 1)
        assert expected_error in "\n".join(policy_errors(
            policy, evidence, candidate_metrics, response_schema, changed_projections))
    admission = AUTHOR_ADMISSION_PATH.as_posix()
    for old, new, expected_error in (
        ("    receipt_ref,\n", "", "Admission ReceiptSettled branch shape drifted"),
        ("      | ReceiptOnly,\n",
         "      | ActivityBacked { project_activity_position },\n",
         "Admission ReceiptSettled branch shape drifted"),
        ("      | ReceiptOnly,\n",
         "      | ReceiptOnly | OptionalActivity,\n",
         "Admission ReceiptSettled branch shape drifted"),
        ("| `AuthoritativeApplied` | `ActivityBacked { project_activity_position }`; exactly one",
         "| `AuthoritativeApplied` | `ReceiptOnly`; exactly one",
         "Admission AuthoritativeApplied Activity mapping drifted"),
        ("| `NoEffect`, `Conflicted`, or `Refused` | `ReceiptOnly`; one typed Receipt and zero Project Activity",
         "| `NoEffect`, `Conflicted`, or `Refused` | `ActivityBacked { project_activity_position }`; one typed Receipt and zero Project Activity",
         "Admission zero-authority Receipt-only mapping drifted"),
        ("| `NoEffect`, `Conflicted`, or `Refused` | `ReceiptOnly`; one typed Receipt and zero Project Activity",
         "| `NoEffect`, `Conflicted`, or `Refused` | `ReceiptOnly`; no typed Receipt and zero Project Activity",
         "Admission zero-authority Receipt-only mapping drifted"),
        ("one-to-one linkage among Admission, Command, and typed Receipt",
         "one-to-one linkage among admission, command,\n  Receipt, and Project Activity",
         "Admission cardinality lost"),
    ):
        changed_projections = dict(projections)
        changed_projections[admission] = changed_projections[admission].replace(old, new, 1)
        assert expected_error in "\n".join(policy_errors(
            policy, evidence, candidate_metrics, response_schema, changed_projections))
    changed_outcome_schema = deepcopy(outcome_schema)
    changed_outcome_schema["$defs"]["ApplyAuthorEditOutcome"]["oneOf"].append({
        "type": "object",
        "additionalProperties": False,
        "properties": {"outcome_kind": {"const": "assumed_success", "type": "string"}},
        "required": ["outcome_kind"],
    })
    assert "public branches drifted" in "\n".join(
        apply_author_edit_outcome_contract_errors(
            changed_outcome_schema, outcome_response_schema, route_catalog, typescript_client,
            web_projection
        )
    )
    changed_outcome_schema = deepcopy(outcome_schema)
    changed_outcome_schema["type"] = "array"
    assert "response envelope drifted" in "\n".join(
        apply_author_edit_outcome_contract_errors(
            changed_outcome_schema, outcome_response_schema, route_catalog, typescript_client,
            web_projection
        )
    )
    changed_outcome_schema = deepcopy(outcome_schema)
    changed_outcome_schema["$defs"]["ApplyAuthorEditOutcome"]["oneOf"][0]["properties"][
        "outcome_kind"
    ]["type"] = "number"
    assert "committed branch drifted" in "\n".join(
        apply_author_edit_outcome_contract_errors(
            changed_outcome_schema, outcome_response_schema, route_catalog, typescript_client,
            web_projection
        )
    )
    changed_outcome_schema = deepcopy(outcome_schema)
    del changed_outcome_schema["$defs"]["ApplyAuthorEditResponse"]["properties"]["command_id"]
    assert "committed response-v2 binding drifted" in "\n".join(
        apply_author_edit_outcome_contract_errors(
            changed_outcome_schema, outcome_response_schema, route_catalog, typescript_client,
            web_projection
        )
    )
    changed_response_schema = deepcopy(outcome_response_schema)
    changed_response_schema["$id"] = "storyos.command.apply-author-edit.response.v3"
    assert "committed response-v2 binding drifted" in "\n".join(
        apply_author_edit_outcome_contract_errors(
            outcome_schema, changed_response_schema, route_catalog, typescript_client,
            web_projection))
    changed_outcome_schema = deepcopy(outcome_schema)
    changed_outcome_schema["$defs"]["ApplyAuthorEditUnknownObservation"]["oneOf"][1][
        "properties"
    ]["reconciliation_required"]["const"] = False
    assert "reconciliation marker drifted" in "\n".join(
        apply_author_edit_outcome_contract_errors(
            changed_outcome_schema, outcome_response_schema, route_catalog, typescript_client,
            web_projection
        )
    )
    for path, value in (
        (("query_proof", "url"), "allowed"),
        (("query_proof", "header"), "X-StoryOS-Proof-In-URL"),
        (("admission",), "command"),
        (("activity", "success"), ["ProjectActivity"]),
    ):
        changed_route_catalog = deepcopy(route_catalog)
        outcome_route = next(operation for operation in changed_route_catalog["operations"]
                             if operation["operation_id"] == "getApplyAuthorEditOutcome")
        target = outcome_route
        for segment in path[:-1]:
            target = target[segment]
        target[path[-1]] = value
        assert "route proof drifted" in "\n".join(apply_author_edit_outcome_contract_errors(
            outcome_schema, outcome_response_schema, changed_route_catalog, typescript_client,
            web_projection))
    for old, new in (
        ("export declare function getApplyAuthorEditOutcome(options: StoryOSQueryOptions & { "
         "projectId: string; idempotencyKey: string; antiForgery: string })",
         "export declare function getApplyAuthorEditOutcome(options: StoryOSQueryOptions & { "
         "projectId: string; idempotencyKey: string; proofInUrl: string })"),
        ('} | { "outcome_kind": "rejected"',
         '} | { "outcome_kind": "assumed_success" } | { "outcome_kind": "rejected"'),
        ('export type ApplyAuthorEditRejectionReason = "challenge_expired_unconsumed";',
         "export type ApplyAuthorEditRejectionReason = string;"),
        ("Promise<GetApplyAuthorEditOutcomeResponse>", "Promise<unknown>"),
    ):
        assert "generated TypeScript drifted" in "\n".join(
            apply_author_edit_outcome_contract_errors(
                outcome_schema, outcome_response_schema, route_catalog,
                typescript_client.replace(old, new, 1), web_projection))
    for old, new, expected_error in (
        ("  | OutcomeQueryUnresolved {", "  | LostOutcomeQueryUnresolved {",
         "GroupReconciliation outcome Query variants drifted"),
        ("      reconciliation_required: true", "      reconciliation_required: false",
         "OutcomeQueryUnresolved reconciliation shape drifted"),
        ("      latest_outcome_query_observation_id | null",
         "      latest_outcome_query_observation_id | null\n      retry_permitted: true",
         "OutcomeQueryUnresolved reconciliation shape drifted"),
        ("| OutcomeQueryRejected { outcome_query_observation_id }",
         "| OutcomeQueryRejected { wrong_outcome_query_observation_id }",
         "outcome Query terminal relation drifted"),
        ("reason: challenge_expired_unconsumed\n      observed_at",
         "reason: challenge_expired_unconsumed\n      observed_at\n      retry_permitted: true",
         "outcome Query rejected settlement relation drifted"),
        ("local_rejected_surface_id\n      preserved_local_payload_ref",
         "local_rejected_surface_id\n      preserved_local_payload_ref\n      saved: true",
         "outcome Query rejected visible relation drifted"),
        ("ApplyAuthorEditOutcomeQueryAttempt =", "LostOutcomeQueryAttempt =",
         "outcome Query attempt shape drifted"),
        ("      route_template: /api/v1/projects/{project_id}/manuscript/author-edit-outcomes/"
         "{idempotency_key}\n", "", "outcome Query attempt shape drifted"),
        ("evidence: delivery_unknown | safe_problem | invalid_envelope",
         "evidence: delivery_unknown | invalid_envelope", "outcome Query attempt shape drifted"),
        ("    local_response_payload_ref\n    exact_response_payload_digest",
         "    local_response_payload_ref\n    lost_response_payload_digest",
         "outcome Query observation shape drifted"),
        ("      | StillUnknown {", "      | AssumedSuccess\n      | StillUnknown {",
         "outcome Query observation shape drifted"),
        ("QueryUnavailable -> preserve strongest_valid_observation",
         "QueryUnavailable -> NoOutcomeObserved", "outcome Query reducer variants drifted"),
        ("ChallengeIssued(exact same expires_at)", "ChallengeIssued(changed expires_at)",
         "outcome Query reducer variants drifted"),
        ("AdmissionCommitted(same Command and Admission) -> AdmissionCommitted | Committed(same identities)",
         "AdmissionCommitted(same Command and Admission) -> AdmissionCommitted | Committed(same identities)\n"
         "  AdmissionCommitted -> Rejected", "outcome Query reducer variants drifted"),
        ("Rejected | Committed -> terminal immutable",
         "Rejected | Committed -> may regress", "outcome Query reducer variants drifted"),
        ("All three records\nname the same exact query observation",
         "The records\nmay use different observations", "outcome Query closed meaning drifted"),
        ("does not consume the nonce", "consumes the nonce",
         "outcome Query closed meaning drifted"),
        ("No\n`Rejected` or `StillUnknown` branch releases the dependent queue",
         "A\n`StillUnknown` branch releases the dependent queue",
         "outcome Query closed meaning drifted"),
        ("does not display saved, rejected, or settled",
         "displays saved before the Journal commit", "outcome Query closed meaning drifted"),
        ("After reload, crash,\nprocess restart, or continued current-process recovery, an "
         "`ApplyAuthorEdit`\n`DeliveryUnknown` state with an available exact capsule and "
         "still-valid Client\nSession permits only the protected outcome Query",
         "After reload, crash,\nprocess restart, or continued current-process recovery, an "
         "`ApplyAuthorEdit`\n`DeliveryUnknown` state with an available exact capsule and "
         "still-valid Client\nSession permits only exact transport replay",
         "outcome Query closed meaning drifted"),
        ("After reload, crash,\nprocess restart, or continued current-process recovery",
         "After reload, process\nrestart, or continued current-process recovery",
         "outcome Query closed meaning drifted"),
        ("Section 10 consumes this same rule; it does not select `ExactTransportReplay`\n"
         "for `ApplyAuthorEdit`.",
         "Section 10 may select `ExactTransportReplay`\nfor `ApplyAuthorEdit`.",
         "outcome Query closed meaning drifted"),
        ("| send may have occurred, crash before response observation commits, "
         "`ApplyAuthorEdit`, capsule available, Client Session binding still exact | "
         "reopen the exact capsule; enter `OutcomeQueryUnresolved { NoOutcomeObserved }` "
         "if not already there; call `getApplyAuthorEditOutcome` first with the stored "
         "key and nonce; do not replay the command, obtain a new challenge, or send again |",
         "| send may have occurred, crash before response observation commits | "
         "reopen the exact capsule and append only `ExactTransportReplay` through the "
         "original command route |\n"
         "| send may have occurred, crash before response observation commits, "
         "`ApplyAuthorEdit`, capsule available, Client Session binding still exact | "
         "reopen the exact capsule; enter `OutcomeQueryUnresolved { NoOutcomeObserved }` "
         "if not already there; call `getApplyAuthorEditOutcome` first with the stored "
         "key and nonce; do not replay the command, obtain a new challenge, or send again |",
         "Web reload matrix still requires ExactTransportReplay for ApplyAuthorEdit"),
        ("| send may have occurred, crash before response observation commits, "
         "`ApplyAuthorEdit`, capsule available, Client Session binding still exact | "
         "reopen the exact capsule; enter `OutcomeQueryUnresolved { NoOutcomeObserved }` "
         "if not already there; call `getApplyAuthorEditOutcome` first with the stored "
         "key and nonce; do not replay the command, obtain a new challenge, or send again |",
         "| send may have occurred, crash before response observation commits, "
         "`ApplyAuthorEdit`, capsule available, Client Session binding still exact | "
         "reopen the exact capsule and append only `ExactTransportReplay` through the "
         "original command route |",
         "Web reload GET-first meaning drifted"),
        ("| send may have occurred, crash before response observation commits, another "
         "editor command, exact capsule and Client Session still valid | reopen the exact "
         "capsule and append only `ExactTransportReplay` through the original command "
         "route |",
         "| send may have occurred, crash before response observation commits, another "
         "editor command, exact capsule and Client Session still valid | call "
         "`getApplyAuthorEditOutcome` first |",
         "Web reload GET-first meaning drifted"),
        ("| `ApplyAuthorEdit` delivery unknown and Client Session binding is "
         "invalid/expired or capsule/request equality is unverifiable | remain "
         "`OutcomeQueryUnresolved { NoOutcomeObserved }`; block replay, fresh challenge, "
         "and dependent submission |",
         "| `ApplyAuthorEdit` delivery unknown and Client Session binding is "
         "invalid/expired or capsule/request equality is unverifiable | remain "
         "`TransportOrAdmissionUnknown`; append `ExactTransportReplay` |",
         "Web reload GET-first meaning drifted"),
        ("including after reload, crash, or process\n    restart.",
         "except after reload, crash, or process\n    restart.",
         "Web reload GET-first meaning drifted"),
        ("for missing `ApplyAuthorEdit` acknowledgement use the protected outcome Query;",
         "for missing `ApplyAuthorEdit` acknowledgement use the available exact replay "
         "capsule;",
         "Web reload GET-first meaning drifted"),
        ("Event arrival alone does not settle the local group",
         "Event arrival alone settles the local group",
         "Web reload GET-first meaning drifted"),
        ("Server or PostgreSQL process interruption does not create a different first\n"
         "browser action. Durable Server facts remain owned by the PostgreSQL Project\n"
         "Storage, Isolation, and Migration Contract. The browser follows the same\n"
         "DeliveryUnknown or durable-observation matrix.",
         "Server or PostgreSQL process interruption may create a different first\n"
         "browser action. Durable Server facts remain owned by the PostgreSQL Project\n"
         "Storage, Isolation, and Migration Contract. The browser follows a distinct\n"
         "server-down first-action matrix.",
         "Web reload GET-first meaning drifted"),
        ("Server or PostgreSQL process interruption uses this same section 10 matrix. It\n"
         "does not create a different first browser action.",
         "Server or PostgreSQL process interruption uses a distinct first browser action.",
         "Web reload GET-first meaning drifted"),
        ("after reload, crash, or process restart; and no second command invocation;",
         "after reload or process restart; and no second command invocation;",
         "Web reload GET-first meaning drifted"),
        ("frozen/unsubmitted journal group, pre-admission refusal, or delivery/Admission "
         "uncertainty | local journal/capsule only; no Host Artifact ingress and no "
         "`RecoveryDraft` claim",
         "frozen/unsubmitted journal group, pre-admission refusal, or delivery/Admission "
         "uncertainty | create a `RecoveryDraft` from reload, journal, or capsule evidence",
         "Web reload GET-first meaning drifted"),
        ("takeover delivery unknown | exact transport replay is allowed only with its "
         "available capsule and current binding; never obtain a fresh challenge until "
         "positive no-Admission proof",
         "takeover delivery unknown | call `getApplyAuthorEditOutcome` first with the "
         "stored key and nonce; do not replay the command",
         "Web reload GET-first meaning drifted"),
        ("applies only to `ApplyAuthorEdit`", "applies to every command",
         "outcome Query closed meaning drifted"),
    ):
        changed_web_projection = web_projection.replace(old, new, 1)
        mutation_errors = apply_author_edit_outcome_contract_errors(
            outcome_schema, outcome_response_schema, route_catalog, typescript_client,
            changed_web_projection
        )
        assert expected_error in "\n".join(mutation_errors), (old, mutation_errors)
    for duplicate in (
        "GroupReconciliation =", "GroupSettlement =", "AuthorSurfaceConvergence =",
        "ApplyAuthorEditOutcomeQueryAttempt =", "ApplyAuthorEditOutcomeQueryObservation =",
        "OutcomeQueryReducer =",
    ):
        assert "definition count drifted" in "\n".join(
            apply_author_edit_outcome_contract_errors(
                outcome_schema, outcome_response_schema, route_catalog, typescript_client,
                f"{web_projection}\n{duplicate}\n"))
def main() -> None:
    errors = policy_errors(*current_sources())
    errors.extend(apply_author_edit_outcome_contract_errors(*current_outcome_sources()))
    if errors:
        raise SystemExit("Author Edit batch policy verification failed:\n- " + "\n- ".join(errors))
    print(
        f"verified structured {REVISION} source, evidence binding, and document projection drift; "
        "semantic redlines run in the browser harness"
    )
if __name__ == "__main__":
    self_test() if sys.argv[1:] == ["--self-test"] else main()
