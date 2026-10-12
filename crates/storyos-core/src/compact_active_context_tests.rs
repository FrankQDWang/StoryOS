use super::{
    ACTIVE_COMPACTION_REQUEST_PREFIX, COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN,
    CompactionInstallFacts, CompactionInstallRefusal, CompactionKnownInput, CompactionRecordState,
    CompactionSource, CompactionStep, HOST_FAKE_COMPACTION_OUTPUT, HOST_FAKE_COMPACTION_PRODUCER,
    StagedCompaction, active_context_input_digest, decide_compaction_install, next_compaction_step,
    requests_active_compaction, stage_host_fake_compaction,
};

fn facts(
    source_restricted: bool,
    exact_required_satisfied: bool,
    staged_input_digest: &str,
    current_input_digest: &str,
) -> CompactionInstallFacts {
    CompactionInstallFacts {
        source_restricted,
        exact_required_satisfied,
        staged_input_digest: staged_input_digest.to_owned(),
        current_input_digest: current_input_digest.to_owned(),
    }
}

#[test]
fn ordinary_assistance_does_not_request_active_compaction() {
    assert!(requests_active_compaction(ACTIVE_COMPACTION_REQUEST_PREFIX));
    assert!(!requests_active_compaction("Help with this passage."));
}

#[test]
fn input_digest_binds_the_exact_message_revision_and_body() {
    assert_eq!(
        active_context_input_digest("Compact active context between calls.", "rev-1", "Passage.",),
        "sha256:e6b1c44b026b3300a9ad38ee204c31fa81ff4e24145cea477caad05c6c2e484f"
    );
}

#[test]
fn install_refuses_a_restricted_changed_or_incomplete_exact_input() {
    let staged = "sha256:staged";
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ false, /*exact_required_satisfied*/ true, staged,
            staged,
        )),
        Ok(())
    );
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ true, /*exact_required_satisfied*/ true, staged,
            staged,
        )),
        Err(CompactionInstallRefusal::RestrictedSource)
    );
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ false,
            /*exact_required_satisfied*/ false,
            staged,
            "sha256:other",
        )),
        Err(CompactionInstallRefusal::ExactRequiredUnsatisfied)
    );
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ false,
            /*exact_required_satisfied*/ true,
            staged,
            "sha256:other",
        )),
        Err(CompactionInstallRefusal::ChangedInput)
    );
}

#[test]
fn a_requested_compaction_stages_once_then_installs_or_refuses_once() {
    let requested = ACTIVE_COMPACTION_REQUEST_PREFIX;
    let states = [
        CompactionRecordState::Absent,
        CompactionRecordState::Staged,
        CompactionRecordState::Installed,
        CompactionRecordState::Refused,
    ];

    assert_eq!(
        (
            states.map(|state| next_compaction_step(requested, state)),
            states.map(|state| next_compaction_step("Help with this passage.", state)),
        ),
        (
            [
                CompactionStep::Stage,
                CompactionStep::InstallOrRefuse,
                CompactionStep::Complete,
                CompactionStep::Complete,
            ],
            [CompactionStep::Complete; 4],
        )
    );
}

#[test]
fn the_fake_result_names_its_known_inputs_and_unknown_preservation() {
    let staged = stage_host_fake_compaction(&CompactionSource {
        model_attempt_id: "attempt-1".to_owned(),
        manifest_id: "manifest-1".to_owned(),
        selected: vec![("working_target".to_owned(), "rev-1".to_owned())],
        item_ids: vec!["item-1".to_owned()],
    });

    assert_eq!(
        staged,
        StagedCompaction {
            producer: HOST_FAKE_COMPACTION_PRODUCER,
            mapping_revision: crate::HOST_FAKE_MAPPING_REVISION,
            output_text: HOST_FAKE_COMPACTION_OUTPUT,
            loss_facts: vec![COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN],
            known_inputs: vec![
                CompactionKnownInput::ModelAttempt {
                    id: "attempt-1".to_owned()
                },
                CompactionKnownInput::Manifest {
                    id: "manifest-1".to_owned()
                },
                CompactionKnownInput::Projection {
                    source_class: "working_target".to_owned(),
                    source_version: "rev-1".to_owned()
                },
            ],
            preserved_item_ids: vec!["item-1".to_owned()],
        }
    );
}
