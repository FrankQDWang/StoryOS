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
    requirement.tickets[11].contract_coverage.requirements =
        &["S1-EVD-001", "S1-EVD-006", "SMAP-STAGE-1"];
    assert!(delivery_error(&requirement).contains("requirement coverage drifted"));

    let mut gate = delivery_contract();
    gate.tickets[11].contract_coverage.gates = &[];
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
        ]
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
            "issue_body_sha256": "bbedc3f73c27611cb688fe7e79662cb62b470343218760af5278b1d1866cf669",
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
