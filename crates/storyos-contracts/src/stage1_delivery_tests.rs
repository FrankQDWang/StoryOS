use crate::stage1_selection::{OPERATION_IDS, proof_selection, requirement_bindings};

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
    evidence.tickets[6].contract_coverage.evidence_classes = &["EV-IT"];
    assert!(delivery_error(&evidence).contains("evidence class coverage drifted"));
}
