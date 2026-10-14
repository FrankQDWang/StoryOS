use serde_json::Value;

use crate::repository_root;

use super::{GENERATED_STAGE4_CROSSWALK_PATH, generate_stage4_crosswalk};

#[test]
fn generated_crosswalk_records_hnd_003_without_stage_release() {
    let bytes = generate_stage4_crosswalk(repository_root())
        .expect("Stage 4 crosswalk generation should succeed");
    let value: Value = serde_json::from_slice(&bytes).expect("generated Stage 4 crosswalk is JSON");
    assert_eq!(value["evaluated_stage"], "Stage 4");
    let handoff = &value["verifications"]["handoff_evidence"];
    assert_eq!(handoff["hnd_003"]["mandatory_map"], "SMAP-STAGE-4");
    assert_eq!(handoff["hnd_004"]["status"], "not-emitted");
    assert_eq!(handoff["emitted"].as_array().map(Vec::len), Some(0));
    let ids = value["declarations"]["bindings"]
        .as_array()
        .expect("bindings should be an array")
        .iter()
        .map(|binding| binding["id"].as_str().expect("id"))
        .collect::<Vec<_>>();
    assert_eq!(ids, super::REQUIRED_IDS);
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
fn checked_in_stage4_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_STAGE4_CROSSWALK_PATH))
        .expect("checked-in Stage 4 crosswalk should exist");
    let expected = generate_stage4_crosswalk(repository_root())
        .expect("Stage 4 crosswalk generation should succeed");
    assert_eq!(actual, expected);
}
