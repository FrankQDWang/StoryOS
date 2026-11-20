use super::*;
use crate::{
    AGENT_PLAN_REGISTRATION, HOST_FAKE_TOOL_LOOP_REGISTRATION, PROBE_FIXTURE_REGISTRATION,
};

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
            function_calling: CapabilityEvidence::Unknown,
        },
        combinations,
        context_window: None,
    }
}

/// A profile of the test with the current entries and no qualified combination: its
/// Registration awaits qualification.
const PENDING_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "test.pending",
    combinations: &[],
    ..AGENT_PLAN_CAPABILITY_PROFILE
};

/// A Registration of the test with the profile `PENDING_PROFILE` and the current requirement.
const PENDING: ModelRegistration = ModelRegistration {
    revision: "test.pending",
    capability_profile: &PENDING_PROFILE,
    ..AGENT_PLAN_REGISTRATION
};

/// A Registration of the test that uses no Host compaction and no retrieval, on the profile
/// `PENDING_PROFILE`.
const NO_COMPACTION: ModelRegistration = ModelRegistration {
    revision: "test.no-compaction",
    requirement: &[
        ModelCapability::Transport,
        ModelCapability::NativeText,
        ModelCapability::Continuation,
        ModelCapability::Streaming,
    ],
    ..PENDING
};

#[test]
fn qualified_members_qualify_only_with_a_qualified_combination() {
    let documented = qualified_entries(&[CapabilityCombination {
        members: COMPACTION_REQUIREMENT,
        evidence: CapabilityEvidence::Documented { source: "test" },
    }]);
    let qualified = qualified_entries(&[CapabilityCombination {
        members: COMPACTION_REQUIREMENT,
        evidence: PROOF,
    }]);

    assert_eq!(
        [
            documented.qualifies(COMPACTION_REQUIREMENT),
            qualified.qualifies(COMPACTION_REQUIREMENT),
            PENDING.qualified(),
            PENDING
                .capability_profile
                .qualifies(&[ModelCapability::Continuation]),
            AGENT_PLAN_REGISTRATION.qualified(),
            AGENT_PLAN_REGISTRATION.compacts(),
            HOST_FAKE_TOOL_LOOP_REGISTRATION.qualified(),
            HOST_FAKE_TOOL_LOOP_REGISTRATION.compacts(),
            NO_COMPACTION.compacts(),
        ],
        [false, true, false, false, true, true, true, true, false]
    );
}

#[test]
fn the_current_profile_records_the_window_of_the_pinned_version() {
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
                window(&PROBE_FIXTURE_REGISTRATION),
                window(&HOST_FAKE_TOOL_LOOP_REGISTRATION),
            ],
            AGENT_PLAN_REGISTRATION.qualified(),
        ),
        ([Some(1_048_576), Some(1_048_576), None], true)
    );
}

#[test]
fn only_a_qualified_profile_or_an_authorized_qualification_of_its_requirement_retrieves() {
    use crate::RuntimeQualification::{QualificationAuthorized, Qualified};

    assert_eq!(
        [
            HOST_FAKE_TOOL_LOOP_REGISTRATION.retrieves(Qualified),
            NO_COMPACTION.retrieves(QualificationAuthorized),
            PENDING.retrieves(Qualified),
            PENDING.retrieves(QualificationAuthorized),
            PENDING.qualified(),
            AGENT_PLAN_REGISTRATION.retrieves(Qualified),
            AGENT_PLAN_REGISTRATION.qualified(),
        ],
        [true, false, false, true, false, true, true]
    );
}

/// ADR 0097: the lookup resolves only the current Registrations. The probe fixture resolves only
/// in test code. Each revision of a removed Registration resolves to nothing.
#[test]
fn only_the_current_registrations_resolve_and_the_head_pins_a_dated_version() {
    let resolved: Vec<_> = (1..=23)
        .map(|number| format!("018f0000-0000-7001-8000-00000000fa{number:02}"))
        .filter(|revision| model_registration(revision).is_some())
        .collect();

    assert_eq!(
        (
            resolved,
            AGENT_PLAN_REGISTRATION.provider_model_id,
            AGENT_PLAN_REGISTRATION.mapping_revision,
            AGENT_PLAN_REGISTRATION.capability_profile.revision,
        ),
        (
            vec![
                "018f0000-0000-7001-8000-00000000fa16".to_owned(),
                "018f0000-0000-7001-8000-00000000fa18".to_owned(),
                "018f0000-0000-7001-8000-00000000fa23".to_owned(),
            ],
            "doubao-seed-2-1-pro-260915",
            "storyos.volcengine-agent-plan.responses.create.v9",
            "storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v4",
        )
    );
}
