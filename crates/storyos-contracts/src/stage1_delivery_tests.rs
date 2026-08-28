use crate::stage1_selection::{OPERATION_IDS, proof_selection, requirement_bindings};
use serde_json::json;

use super::{DeliveryContract, delivery_contract, delivery_coverage};

fn delivery_error(contract: &DeliveryContract) -> String {
    delivery_coverage(
        contract,
        &requirement_bindings(),
        OPERATION_IDS,
        &proof_selection(),
    )
    .expect_err("stale delivery binding should fail closed")
}

#[test]
fn stale_ticket_body_binding_is_rejected() {
    let mut contract = delivery_contract();
    contract.tickets[1].issue_body_sha256 =
        "0000000000000000000000000000000000000000000000000000000000000000";

    assert!(delivery_error(&contract).contains("ticket binding drifted"));
}

#[test]
fn stale_requirement_gate_and_evidence_bindings_are_rejected() {
    let mut requirement = delivery_contract();
    requirement.tickets[12].contract_coverage.requirements =
        &["S1-EVD-001", "S1-EVD-006", "SMAP-STAGE-1"];
    assert!(delivery_error(&requirement).contains("requirement coverage drifted"));

    let mut gate = delivery_contract();
    gate.tickets[12].contract_coverage.gates = &[];
    assert!(delivery_error(&gate).contains("gate coverage drifted"));

    let mut evidence = delivery_contract();
    evidence.tickets[0].contract_coverage.evidence_classes = &["EC-FOREIGN"];
    assert!(delivery_error(&evidence).contains("evidence class coverage drifted"));
}

#[test]
fn delivery_ticket_blockers_serialize_as_closed_arrays() {
    let value = serde_json::to_value(delivery_contract()).expect("delivery contract serializes");
    let blockers: Vec<_> = value["tickets"]
        .as_array()
        .expect("delivery tickets are an array")
        .iter()
        .map(|ticket| ticket["blocked_by"].clone())
        .collect();

    assert_eq!(
        blockers,
        vec![
            json!([]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/103"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/104"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/105"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/119"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/121"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/106"]),
            json!([]),
            json!([
                "https://github.com/FrankQDWang/StoryOS/issues/103",
                "https://github.com/FrankQDWang/StoryOS/issues/108",
                "https://github.com/FrankQDWang/StoryOS/issues/56",
                "https://github.com/FrankQDWang/StoryOS/issues/60"
            ]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/109"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/110"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/111"]),
            json!(["https://github.com/FrankQDWang/StoryOS/issues/209"]),
        ]
    );
}

#[test]
fn protected_web_client_toolchain_ticket_matches_the_current_delivery_lock() {
    let contract = serde_json::to_value(delivery_contract()).expect("delivery contract serializes");
    let tickets = contract["tickets"]
        .as_array()
        .expect("delivery tickets are an array");
    let toolchain = tickets
        .iter()
        .find(|ticket| ticket["issue"] == "https://github.com/FrankQDWang/StoryOS/issues/209")
        .expect("Stage 1 delivery must name the Protected Web Client toolchain Issue");
    let handoff = tickets
        .iter()
        .find(|ticket| ticket["issue"] == "https://github.com/FrankQDWang/StoryOS/issues/112")
        .expect("Stage 1 delivery must keep the evidence handoff Issue");

    assert_eq!(
        toolchain,
        &json!({
            "sequence": 12,
            "responsibility_id": "S1-TICKET-09A",
            "issue": "https://github.com/FrankQDWang/StoryOS/issues/209",
            "parent": "https://github.com/FrankQDWang/StoryOS/issues/100",
            "title": "Build the Protected Web Client with pnpm, Vite, Node, and React",
            "issue_body_sha256": "18316235c10aa76ac35ee976bc05b48a3762c76b3033efccbcfc4c943c89708f",
            "blocked_by": ["https://github.com/FrankQDWang/StoryOS/issues/111"],
            "evidence_role": "planned-runtime-evidence",
            "responsibility": "lock the Protected Web Client pnpm/Vite/React toolchain and Stage 1 view host",
            "contract_coverage": {
                "requirements": [],
                "operations": [],
                "gates": [],
                "evidence_classes": [],
                "fixtures": [],
                "fault_points": [],
                "schedules": [],
                "oracles": [],
                "bundles": [],
                "aggregate": null
            }
        })
    );
    assert_eq!(
        handoff,
        &json!({
            "sequence": 13,
            "responsibility_id": "S1-TICKET-10",
            "issue": "https://github.com/FrankQDWang/StoryOS/issues/112",
            "parent": "https://github.com/FrankQDWang/StoryOS/issues/100",
            "title": "Complete the Stage 1 Mandatory Evidence and Handoff",
            "issue_body_sha256": "e0d15d2cb1bc8e1d1760e20dff8c4c9c9781fe7eab90e8c7b98aab7a7d47517c",
            "blocked_by": ["https://github.com/FrankQDWang/StoryOS/issues/209"],
            "evidence_role": "acceptance-handoff",
            "responsibility": "assemble the mandatory Stage 1 evidence and handoff",
            "contract_coverage": {
                "requirements": ["S1-REQ-006", "S1-EVD-001", "S1-EVD-006", "SMAP-STAGE-1"],
                "operations": [],
                "gates": ["DVG-13"],
                "evidence_classes": [],
                "fixtures": ["FX-HANDOFF"],
                "fault_points": [],
                "schedules": [],
                "oracles": [],
                "bundles": ["B-HANDOFF"],
                "aggregate": "B-S1-MANDATORY-SET"
            }
        })
    );
}

#[test]
fn stage1_handoff_delivery_lock_matches_the_composed_main() {
    let contract = serde_json::to_value(delivery_contract()).expect("delivery contract serializes");
    assert_eq!(
        json!({
            "revision": contract["revision"],
            "baseline_commit": contract["baseline_commit"],
            "baseline_tree": contract["baseline_tree"],
        }),
        json!({
            "revision": "stage1-ticketed-delivery-2026-08-28-v22",
            "baseline_commit": "3523494b0b4d934032576b822895b8f3ebc7ef87",
            "baseline_tree": "5e155a851b1debf555e1d5f20fde09956b609c5d",
        })
    );
}

#[test]
fn acknowledgement_loss_ticket_matches_the_current_process_profile() {
    let contract = serde_json::to_value(delivery_contract()).expect("delivery contract serializes");

    assert_eq!(
        contract["tickets"][8],
        json!({
            "sequence": 9,
            "responsibility_id": "S1-TICKET-07",
            "issue": "https://github.com/FrankQDWang/StoryOS/issues/109",
            "parent": "https://github.com/FrankQDWang/StoryOS/issues/100",
            "title": "Reconcile Acknowledgement Loss without Duplicate Authority",
            "issue_body_sha256": "e349093d8c79e93b51fc6b44dfd877b4fdd9cf0f238821b24d97f480e6842e2a",
            "blocked_by": [
                "https://github.com/FrankQDWang/StoryOS/issues/103",
                "https://github.com/FrankQDWang/StoryOS/issues/108",
                "https://github.com/FrankQDWang/StoryOS/issues/56",
                "https://github.com/FrankQDWang/StoryOS/issues/60"
            ],
            "evidence_role": "planned-runtime-evidence",
            "responsibility": "reconcile acknowledgement loss without duplicate authority",
            "contract_coverage": {
                "requirements": ["S1-REQ-004", "S1-EVD-004"],
                "operations": [
                    "createProjectCommandChallenge",
                    "applyAuthorEdit",
                    "getApplyAuthorEditOutcome"
                ],
                "gates": ["DVG-01", "DVG-02", "DVG-03", "DVG-08", "DVG-11"],
                "evidence_classes": [
                    "EC-01", "EC-02", "EC-03", "EC-04", "EC-05", "EV-CP", "EV-IT",
                    "EV-INT", "EV-SE"
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
                    "CFP-SCOPE-BEFORE-QUERY"
                ],
                "schedules": ["SCH-NORMAL", "SCH-REORDER", "SCH-SCOPE", "SCH-UNKNOWN"],
                "oracles": [
                    "ORC-ATOMIC-AUTHORITY",
                    "ORC-CONTRACT",
                    "ORC-EDITOR-JOURNAL",
                    "ORC-NEGATIVE-CLOSURE",
                    "ORC-OUTCOME-UNKNOWN",
                    "ORC-SCOPE"
                ],
                "bundles": ["B-CONTRACT", "B-CORE", "B-EDITOR", "B-SCOPE"],
                "aggregate": null
            }
        })
    );
}

#[test]
fn acknowledgement_loss_global_selection_is_exact_and_ordered() {
    let proof = proof_selection();

    assert_eq!(
        json!({
            "operations": OPERATION_IDS,
            "fault_points": proof.fault_points,
            "schedules": proof.schedules
        }),
        json!({
            "operations": [
                "getProtocolProfile",
                "getProject",
                "getChapter",
                "createEditorSession",
                "getEditorSession",
                "createProjectCommandChallenge",
                "applyAuthorEdit",
                "getApplyAuthorEditOutcome",
                "getCommand",
                "activityStream",
                "getSnapshot"
            ],
            "fault_points": [
                "CFP-ADMISSION-BEFORE-CORE",
                "CFP-ADMISSION-EXPIRY",
                "CFP-CONTRACT-DRIFT",
                "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
                "CFP-CORE-BEFORE-COMMIT",
                "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
                "CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP",
                "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
                "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
                "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
                "CFP-EDITOR-BEFORE-JOURNAL-DURABILITY",
                "CFP-FENCE-AFTER-TAKEOVER",
                "CFP-LATE-RESULT",
                "CFP-REPLAY-AFTER-GENERATION-SNAPSHOT",
                "CFP-REPLAY-BELOW-FLOOR",
                "CFP-SCOPE-BEFORE-QUERY"
            ],
            "schedules": [
                "SCH-CRASH",
                "SCH-DRIFT",
                "SCH-FENCE",
                "SCH-NORMAL",
                "SCH-REORDER",
                "SCH-REPLAY",
                "SCH-SCOPE",
                "SCH-UNKNOWN"
            ]
        })
    );
}
