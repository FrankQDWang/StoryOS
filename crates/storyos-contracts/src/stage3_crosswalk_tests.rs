use serde_json::Value;

use crate::repository_root;

use super::{GENERATED_STAGE3_CROSSWALK_PATH, generate_stage3_crosswalk};

fn generated() -> Value {
    let bytes = generate_stage3_crosswalk(repository_root())
        .expect("Stage 3 crosswalk generation should succeed");
    serde_json::from_slice(&bytes).expect("generated Stage 3 crosswalk is JSON")
}

#[test]
fn generated_crosswalk_records_hnd_003_without_stage_release() {
    let value = generated();
    assert_eq!(
        value["schema_id"],
        "storyos.evidence.stage3-contract-crosswalk.v1"
    );
    assert_eq!(value["evaluated_stage"], "Stage 3");
    assert_eq!(
        value["claim_ceiling"],
        "implementation-evidence-completeness; no EV-SR; no PASS-STAGE"
    );
    assert_eq!(
        value["verifications"]["handoff_evidence"]["hnd_003"]["evaluated_stage"],
        "Stage 3"
    );
    assert_eq!(
        value["verifications"]["handoff_evidence"]["hnd_003"]["mandatory_map"],
        "SMAP-STAGE-3"
    );
    assert_eq!(
        value["verifications"]["handoff_evidence"]["hnd_004"]["status"],
        "not-emitted"
    );
    let emitted = value["verifications"]["handoff_evidence"]["emitted"]
        .as_array()
        .expect("emitted should be an array");
    assert!(emitted.is_empty());
    let forbidden = value["verifications"]["handoff_evidence"]["forbidden_emissions"]
        .as_array()
        .expect("forbidden emissions should be an array");
    assert!(forbidden.iter().any(|item| item == "EV-SR"));
    assert!(forbidden.iter().any(|item| item == "PASS-STAGE"));
    let classes = value["declarations"]["bindings"]
        .as_array()
        .expect("bindings should be an array")
        .iter()
        .map(|binding| binding["evidence_class"].as_str().expect("class"))
        .collect::<Vec<_>>();
    assert_eq!(classes.len(), 25);
    assert!(classes.contains(&"contract"));
    assert!(classes.contains(&"integration"));
    assert!(classes.contains(&"physical_recovery"));
    assert!(classes.contains(&"stage"));
}

#[test]
fn binding_evidence_paths_exist_in_the_worktree() {
    let root = repository_root();
    for binding in super::BINDINGS {
        for path in
            std::iter::once(binding.evidence).chain(binding.supporting_evidence.iter().copied())
        {
            assert!(
                root.join(path).is_file(),
                "missing evidence file {path} for {}",
                binding.id
            );
        }
    }
}

#[test]
fn missing_rel_007_is_rejected() {
    let mut bindings = super::BINDINGS.to_vec();
    bindings[0].id = "NOT-REL-007";
    let handoff = super::HandoffEvidence {
        hnd_003: super::Hnd003 {
            id: "HND-003",
            evaluated_stage: "Stage 3",
            mandatory_map: "SMAP-STAGE-3",
            disposition: "implementation-evidence-completeness; no EV-SR; no PASS-STAGE",
        },
        hnd_004: super::DeferredHandoff {
            id: "HND-004",
            status: "not-emitted",
        },
        emitted: &[],
        forbidden_emissions: &["EV-SR", "PASS-STAGE", "PASS-CLOUD"],
    };
    assert_eq!(
        super::verify_stage3_record(&bindings, &handoff).unwrap_err(),
        "Stage 3 binding REL-007 is missing or reordered"
    );
}

#[test]
fn checked_in_stage3_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_STAGE3_CROSSWALK_PATH))
        .expect("checked-in Stage 3 crosswalk should exist");
    let expected = generate_stage3_crosswalk(repository_root())
        .expect("Stage 3 crosswalk generation should succeed");
    assert_eq!(actual, expected);
}
