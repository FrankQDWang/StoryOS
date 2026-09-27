use super::{
    ACTIVE_COMPACTION_REQUEST_PREFIX, CompactionInstallFacts, CompactionInstallRefusal,
    active_context_input_digest, decide_compaction_install, requests_active_compaction,
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
