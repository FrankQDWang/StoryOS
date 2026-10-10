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
            AGENT_PLAN_CAPABILITY_PROFILE.qualifies(CREATE_REQUIREMENT),
        ],
        [false, true, false]
    );
}
