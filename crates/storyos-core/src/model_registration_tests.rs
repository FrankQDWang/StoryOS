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
            AGENT_PLAN_REGISTRATION.qualified(),
            HOST_FAKE_REGISTRATION.qualified(),
        ],
        [
            false, true, false, true, false, false, true, true, false, true, true, true
        ]
    );
}
