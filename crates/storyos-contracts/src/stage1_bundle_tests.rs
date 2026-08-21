use super::{mandatory_evidence, verify_mandatory_evidence};

fn complete_evidence() -> super::MandatoryEvidence {
    mandatory_evidence().expect("complete mandatory evidence should verify")
}

#[test]
fn missing_mandatory_set_member_is_rejected() {
    let mut evidence = complete_evidence();
    evidence.b_s1_mandatory_set.members = &[
        "B-CONTRACT",
        "B-SCOPE",
        "B-EDITOR",
        "B-CORE",
        "B-RECOVERY",
        "B-HANDOFF",
    ];
    let error =
        verify_mandatory_evidence(&evidence).expect_err("missing member should fail closed");
    assert!(
        error.contains("B-S1-MANDATORY-SET missing member"),
        "{error}"
    );
    assert!(error.contains("B-REPLAY"), "{error}");
}

#[test]
fn excluded_stage2_bundle_is_rejected() {
    let mut evidence = complete_evidence();
    evidence.b_s1_mandatory_set.members = &[
        "B-CONTRACT",
        "B-SCOPE",
        "B-EDITOR",
        "B-CORE",
        "B-RECOVERY",
        "B-REPLAY",
        "B-RESTORE",
    ];
    let error =
        verify_mandatory_evidence(&evidence).expect_err("excluded bundle should fail closed");
    assert_eq!(
        error,
        "B-S1-MANDATORY-SET contains excluded bundle B-RESTORE"
    );
}

#[test]
fn stale_fx_handoff_delivery_baseline_is_rejected() {
    let mut evidence = complete_evidence();
    evidence.fx_handoff.delivery_baseline.tree = "wrong-tree";
    let error =
        verify_mandatory_evidence(&evidence).expect_err("stale baseline should fail closed");
    assert_eq!(error, "FX-HANDOFF delivery baseline drifted");
}

#[test]
fn smap_stage_1_rejects_stage_release_evidence() {
    let mut evidence = complete_evidence();
    evidence.smap_stage_1.evidence_classes = &[
        "EC-01", "EC-02", "EC-03", "EC-04", "EC-05", "EC-07", "EV-CP", "EV-IT", "EV-INT", "EV-SE",
        "EV-SR",
    ];
    let error = verify_mandatory_evidence(&evidence).expect_err("EV-SR should fail closed");
    assert_eq!(error, "SMAP-STAGE-1 must not contain EV-SR");
}
