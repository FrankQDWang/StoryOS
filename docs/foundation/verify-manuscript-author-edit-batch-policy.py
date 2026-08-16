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
import sys


ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = ROOT / "docs/foundation/author-edit-batch-release-1-policy.json"
EVIDENCE_PATH = ROOT / "docs/research/author-edit-batch-prerelease-browser-evidence.json"
PROJECTIONS = (
    ROOT / "CONTEXT.md",
    ROOT / "docs/adr/0003-specify-manuscript-revision-and-proposal-state-machine.md",
    ROOT / "docs/foundation/manuscript-revision-proposal-state-machine.md",
    ROOT / "docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md",
    ROOT / "docs/foundation/deterministic-verification-and-failure-recovery-gates.md",
)
REVISION = "storyos.author-edit-batch.release-1.preview.v1"
EDITOR_REVISION = "storyos.editor-contract.release-1.v2"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def current_sources() -> tuple[dict, dict, dict[str, str]]:
    return (
        json.loads(POLICY_PATH.read_text(encoding="utf-8")),
        json.loads(EVIDENCE_PATH.read_text(encoding="utf-8")),
        {path.as_posix(): path.read_text(encoding="utf-8") for path in PROJECTIONS},
    )


def policy_errors(policy: dict, evidence: dict, projections: dict[str, str]) -> list[str]:
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
    if not evidence.get("semantic_oracle") or not all(evidence["semantic_oracle"].values()):
        errors.append("captured semantic oracle is incomplete or failed")
    adoption = evidence.get("adoption", {})
    if adoption.get("decision") != "adopt_conservative_prerelease" \
            or adoption.get("idle_ms") != selected.get("adjacent_completed_intent_idle_ms") \
            or adoption.get("max_units") != selected.get("max_author_edit_units"):
        errors.append("captured adoption does not match the selected prerelease policy")
    combined = "\n".join(projections.values())
    if "storyos.author-edit-batch.release-1.v1" in combined:
        errors.append("superseded stable-looking policy identity remains in a projection")
    for required in (REVISION, EDITOR_REVISION, "author-edit-batch-release-1-policy.json"):
        if required not in combined:
            errors.append(f"document projections lost {required}")
    return errors


def self_test() -> None:
    policy, evidence, projections = current_sources()
    assert policy_errors(policy, evidence, projections) == []
    for path, value in (
        (("selected", "max_author_edit_units"), 999),
        (("identity_binding", "wire_change"), True),
        (("receipt_boundary", "receipt_free"), ["refused"]),
    ):
        changed = deepcopy(policy)
        changed[path[0]][path[1]] = value
        assert policy_errors(changed, evidence, projections)
    changed_evidence = deepcopy(evidence)
    changed_evidence["semantic_oracle"]["whole_batch_has_one_final_settlement"] = False
    assert policy_errors(policy, changed_evidence, projections)
    changed_projections = dict(projections)
    first = next(iter(changed_projections))
    changed_projections[first] += "\nstoryos.author-edit-batch.release-1.v1\n"
    assert policy_errors(policy, evidence, changed_projections)


def main() -> None:
    errors = policy_errors(*current_sources())
    if errors:
        raise SystemExit("Author Edit batch policy verification failed:\n- " + "\n- ".join(errors))
    print(
        f"verified structured {REVISION} source, evidence binding, and document projection drift; "
        "semantic redlines run in the browser harness"
    )


if __name__ == "__main__":
    self_test() if sys.argv[1:] == ["--self-test"] else main()
