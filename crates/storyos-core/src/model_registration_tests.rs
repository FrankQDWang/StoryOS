use super::*;

const PROOF: CapabilityEvidence = CapabilityEvidence::Qualified { evidence: "test" };

fn qualified_entries(combinations: &'static [CapabilityCombination]) -> ModelCapabilityProfile {
    ModelCapabilityProfile {
        revision: "test",
        entries: CapabilityEntries {
            transport: PROOF,
            native_text: PROOF,
            structured_text: PROOF,
            continuation: PROOF,
            streaming: PROOF,
            implicit_cache: PROOF,
            explicit_cache: PROOF,
            native_compaction: PROOF,
            host_compaction: PROOF,
            retrieval: PROOF,
            abort: PROOF,
        },
        combinations,
        context_window: None,
    }
}

#[test]
fn qualified_members_qualify_only_with_a_qualified_combination() {
    let documented = qualified_entries(&[CapabilityCombination {
        members: CREATE_REQUIREMENT,
        evidence: CapabilityEvidence::Documented { source: "test" },
    }]);
    let qualified = qualified_entries(&[CapabilityCombination {
        members: CREATE_REQUIREMENT,
        evidence: PROOF,
    }]);

    assert_eq!(
        [
            documented.qualifies(CREATE_REQUIREMENT),
            qualified.qualifies(CREATE_REQUIREMENT),
            AGENT_PLAN_PENDING_REGISTRATION.qualified(),
            AGENT_PLAN_CREATE_REGISTRATION.qualified(),
            AGENT_PLAN_CREATE_CAPABILITY_PROFILE.qualifies(&[ModelCapability::Continuation]),
            AGENT_PLAN_CONTINUATION_PENDING_REGISTRATION.qualified(),
            AGENT_PLAN_CONTINUATION_PENDING_REGISTRATION
                .capability_profile
                .qualifies(CREATE_REQUIREMENT),
            AGENT_PLAN_CONTINUATION_REGISTRATION.qualified(),
            AGENT_PLAN_STREAMING_PENDING_REGISTRATION.qualified(),
            AGENT_PLAN_STREAMING_PENDING_REGISTRATION
                .capability_profile
                .qualifies(CONTINUATION_REQUIREMENT),
            AGENT_PLAN_STREAMING_REGISTRATION.qualified(),
            AGENT_PLAN_COMPACTION_PENDING_REGISTRATION.qualified(),
            AGENT_PLAN_COMPACTION_PENDING_REGISTRATION
                .capability_profile
                .qualifies(STREAMING_REQUIREMENT),
            AGENT_PLAN_REGISTRATION.qualified(),
            AGENT_PLAN_REGISTRATION.compacts(),
            HOST_FAKE_REGISTRATION.qualified(),
            HOST_FAKE_REGISTRATION.compacts(),
            AGENT_PLAN_STREAMING_REGISTRATION.compacts(),
        ],
        [
            false, true, false, true, false, false, true, true, false, true, true, false, true,
            true, true, true, true, false
        ]
    );
}

#[test]
fn the_head_profile_records_the_window_and_the_pending_profile_does_not_qualify_it() {
    let window = |registration: &ModelRegistration| {
        registration
            .capability_profile
            .context_window
            .map(|window| window.tokens)
    };

    assert_eq!(
        (
            [
                window(&AGENT_PLAN_REGISTRATION),
                window(&AGENT_PLAN_WINDOW_PENDING_REGISTRATION),
                window(&AGENT_PLAN_COMPACTION_REGISTRATION),
                window(&HOST_FAKE_REGISTRATION),
            ],
            AGENT_PLAN_WINDOW_PENDING_REGISTRATION.qualified(),
            AGENT_PLAN_REGISTRATION.qualified(),
        ),
        ([Some(262_144), Some(262_144), None, None], false, true)
    );
}

#[test]
fn only_a_qualified_profile_or_an_authorized_qualification_of_its_requirement_retrieves() {
    use crate::RuntimeQualification::{Qualified, QualificationAuthorized};

    assert_eq!(
        [
            HOST_FAKE_REGISTRATION.retrieves(Qualified),
            AGENT_PLAN_THRESHOLD_REGISTRATION.retrieves(QualificationAuthorized),
            AGENT_PLAN_RECOVERY_PENDING_REGISTRATION.retrieves(Qualified),
            AGENT_PLAN_RECOVERY_PENDING_REGISTRATION.retrieves(QualificationAuthorized),
            AGENT_PLAN_RECOVERY_PENDING_REGISTRATION.qualified(),
            AGENT_PLAN_REGISTRATION.retrieves(Qualified),
            AGENT_PLAN_REGISTRATION.qualified(),
        ],
        [true, false, false, true, false, true, true]
    );
}
