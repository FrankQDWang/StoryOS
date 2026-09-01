use serde_json::Value;

use crate::repository_root;

use super::{GENERATED_STAGE2_CROSSWALK_PATH, generate_stage2_crosswalk};

const REQUIRED_IDS: &[&str] = &[
    "REL-007",
    "S2-REQ-001",
    "S2-REQ-002",
    "S2-REQ-003",
    "S2-REQ-004",
    "S2-REQ-005",
    "S2-REQ-006",
    "S2-REQ-007",
    "S2-REQ-008",
    "S2-REQ-009",
    "S2-EVD-001",
    "S2-EVD-002",
    "S2-EVD-003",
    "S2-EVD-004",
    "S2-EVD-005",
    "S2-EVD-006",
    "S2-EVD-007",
    "S2-EVD-008",
    "S2-EVD-009",
    "S2-JRN-001",
    "S2-JRN-001/1",
    "S2-JRN-001/2",
    "S2-JRN-001/3",
    "S2-JRN-001/4",
    "S2-JRN-001/5",
    "S2-JRN-001/6",
    "S2-JRN-001/7",
    "S2-JRN-001/8",
    "S2-JRN-001/9",
    "S2-JRN-001/10",
    "S2-JRN-001/11",
    "S2-JRN-001/12",
];
const EVIDENCE_CLASSES: &[&str] = &["contract", "integration", "physical_recovery", "stage"];

fn generated() -> Value {
    let bytes = generate_stage2_crosswalk(repository_root())
        .expect("Stage 2 crosswalk generation should succeed");
    serde_json::from_slice(&bytes).expect("generated Stage 2 crosswalk is JSON")
}

#[test]
fn generated_crosswalk_records_hnd_003_without_stage_release() {
    let value = generated();
    assert_eq!(
        value["schema_id"],
        "storyos.evidence.stage2-contract-crosswalk.v1"
    );
    assert_eq!(value["evaluated_stage"], "Stage 2");
    assert_eq!(
        value["claim_ceiling"],
        "implementation-evidence-completeness; no EV-SR; no PASS-STAGE"
    );
    assert_eq!(
        value["verifications"]["handoff_evidence"]["hnd_003"]["evaluated_stage"],
        "Stage 2"
    );
    assert_eq!(
        value["verifications"]["handoff_evidence"]["hnd_003"]["mandatory_map"],
        "SMAP-STAGE-2"
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
}

#[test]
fn generated_crosswalk_covers_rel_requirements_evidence_and_journey_steps() {
    let value = generated();
    let bindings = value["declarations"]["bindings"]
        .as_array()
        .expect("bindings should be an array");
    let ids: Vec<&str> = bindings
        .iter()
        .map(|binding| {
            binding["id"]
                .as_str()
                .expect("binding id should be a string")
        })
        .collect();
    for required in REQUIRED_IDS {
        assert!(ids.contains(required), "missing binding {required}");
    }
    assert_eq!(ids.len(), REQUIRED_IDS.len());
}

#[test]
fn generated_crosswalk_keeps_evidence_classes_distinct_and_attributable() {
    let value = generated();
    let bindings = value["declarations"]["bindings"]
        .as_array()
        .expect("bindings should be an array");
    let mut seen_classes = Vec::new();
    for binding in bindings {
        let class = binding["evidence_class"]
            .as_str()
            .expect("evidence_class should be a string");
        assert!(
            EVIDENCE_CLASSES.contains(&class),
            "unknown evidence class {class}"
        );
        if !seen_classes.contains(&class) {
            seen_classes.push(class);
        }
        let issues = binding["implementation_issues"]
            .as_array()
            .expect("implementation_issues should be an array");
        assert!(
            !issues.is_empty(),
            "{} has no implementation owner",
            binding["id"]
        );
        let evidence = binding["evidence"]
            .as_str()
            .expect("evidence should be a string");
        assert!(
            !evidence.is_empty(),
            "{} has no evidence path",
            binding["id"]
        );
        let owners = binding["owners"]
            .as_array()
            .expect("owners should be an array");
        assert!(
            !owners.is_empty(),
            "{} has no semantic owner",
            binding["id"]
        );
    }
    for required_class in EVIDENCE_CLASSES {
        assert!(
            seen_classes.contains(required_class),
            "missing evidence class {required_class}"
        );
    }
}

#[test]
fn binding_evidence_paths_exist_in_the_worktree() {
    let root = repository_root();
    for binding in super::BINDINGS {
        assert!(
            root.join(binding.evidence).is_file(),
            "missing evidence file {} for {}",
            binding.evidence,
            binding.id
        );
    }
}

#[test]
fn missing_rel_007_is_rejected() {
    let mut bindings = super::BINDINGS.to_vec();
    bindings[0].id = "NOT-REL-007";
    let handoff = super::HandoffEvidence {
        hnd_003: super::Hnd003 {
            id: "HND-003",
            evaluated_stage: "Stage 2",
            mandatory_map: "SMAP-STAGE-2",
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
        super::verify_stage2_record(&bindings, &handoff).unwrap_err(),
        "Stage 2 binding REL-007 is missing or reordered"
    );
}

#[test]
fn checked_in_stage2_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_STAGE2_CROSSWALK_PATH))
        .expect("checked-in Stage 2 crosswalk should exist");
    let expected = generate_stage2_crosswalk(repository_root())
        .expect("Stage 2 crosswalk generation should succeed");
    assert_eq!(actual, expected);
}
