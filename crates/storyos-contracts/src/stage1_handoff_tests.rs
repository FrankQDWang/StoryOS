use crate::repository_root;
use crate::stage1_bundle::mandatory_evidence;
use crate::stage1_provenance::provenance_evidence;

use super::{handoff_evidence, verify_handoff_evidence};

fn complete_inputs() -> (
    crate::stage1_bundle::MandatoryEvidence,
    crate::stage1_provenance::ProvenanceEvidence,
) {
    (
        mandatory_evidence().expect("mandatory evidence"),
        provenance_evidence(repository_root()).expect("provenance evidence"),
    )
}

#[test]
fn emitted_stage_release_is_rejected() {
    let (mandatory, provenance) = complete_inputs();
    let mut evidence = handoff_evidence(&mandatory, &provenance).expect("complete evidence");
    evidence.emitted = &["EV-SR"];
    let error = verify_handoff_evidence(&evidence, &mandatory, &provenance)
        .expect_err("emitting EV-SR should fail closed");
    assert!(error.contains("EV-SR"), "{error}");
}

#[test]
fn closed_parent_is_rejected() {
    let (mut mandatory, provenance) = complete_inputs();
    let mut evidence = handoff_evidence(&mandatory, &provenance).expect("complete evidence");
    evidence.parent_issue.state = "closed";
    mandatory.fx_handoff.parent_issue.state = "closed";
    let error = verify_handoff_evidence(&evidence, &mandatory, &provenance)
        .expect_err("closed parent #100 should fail closed");
    assert!(error.contains("parent #100"), "{error}");
}

#[test]
fn missing_later_stage_map_is_rejected() {
    let (mandatory, provenance) = complete_inputs();
    let mut evidence = handoff_evidence(&mandatory, &provenance).expect("complete evidence");
    evidence.out_of_scope.later_stage_maps = &["SMAP-STAGE-2", "SMAP-STAGE-4"];
    let error = verify_handoff_evidence(&evidence, &mandatory, &provenance)
        .expect_err("missing Stage 3 map should fail closed");
    assert!(error.contains("later-stage"), "{error}");
}

#[test]
fn missing_reference_exclusion_is_rejected() {
    let (mandatory, mut provenance) = complete_inputs();
    let evidence = handoff_evidence(&mandatory, &provenance).expect("complete evidence");
    provenance.workspace.excluded_trees = &["prototypes/**"];
    let error = verify_handoff_evidence(&evidence, &mandatory, &provenance)
        .expect_err("missing .reference/** exclusion should fail closed");
    assert!(error.contains(".reference/**"), "{error}");
}
