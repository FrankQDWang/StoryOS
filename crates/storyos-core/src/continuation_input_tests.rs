use super::{
    ContinuationIdentity, ContinuationInputMapping, ContinuationMappingInput,
    map_continuation_input,
};
use crate::HOST_FAKE_MAPPING_REVISION;

fn identity(conversation: &str, destination: &str) -> ContinuationIdentity {
    ContinuationIdentity {
        owner_user_id: "018f0000-0000-7001-8000-000000000001".to_owned(),
        project_id: "018f0000-0000-7001-8000-000000000201".to_owned(),
        conversation_id: conversation.to_owned(),
        destination_identity: destination.to_owned(),
        evidence_revision: "1".to_owned(),
        registration: "018f0000-0000-7001-8000-00000000fa01".to_owned(),
        adapter_mapping: HOST_FAKE_MAPPING_REVISION.to_owned(),
        use_binding: "018f0000-0000-7001-8000-00000000fb01".to_owned(),
        compatibility: "018f0000-0000-7001-8000-00000000fc01".to_owned(),
        covered_copy_restricted: false,
    }
}

fn classify(
    prior: Option<ContinuationIdentity>,
    mapping_can_represent: bool,
) -> ContinuationInputMapping {
    map_continuation_input(&ContinuationMappingInput {
        current: identity(
            "018f0000-0000-7001-8000-000000000a36",
            "018f0000-0000-7001-8000-00000000fd01",
        ),
        prior,
        mapping_can_represent,
    })
}

fn matching_prior() -> ContinuationIdentity {
    identity(
        "018f0000-0000-7001-8000-000000000a36",
        "018f0000-0000-7001-8000-00000000fd01",
    )
}

#[test]
fn first_run_and_matching_prior_choose_the_declared_mapping() {
    assert_eq!(
        classify(None, /*mapping_can_represent*/ true),
        ContinuationInputMapping::None
    );
    assert_eq!(
        classify(Some(matching_prior()), /*mapping_can_represent*/ true),
        ContinuationInputMapping::Incremental
    );
    assert_eq!(
        classify(Some(matching_prior()), /*mapping_can_represent*/ false),
        ContinuationInputMapping::Full
    );
}

#[test]
fn identity_mismatch_or_covered_copy_uses_new_transport() {
    let current = matching_prior();
    let cases = [
        identity(
            "018f0000-0000-7001-8000-000000000b36",
            "018f0000-0000-7001-8000-00000000fd01",
        ),
        identity(
            "018f0000-0000-7001-8000-000000000a36",
            "018f0000-0000-7001-8000-00000000dead",
        ),
        ContinuationIdentity {
            registration: "018f0000-0000-7001-8000-00000000dead".to_owned(),
            ..current.clone()
        },
        ContinuationIdentity {
            covered_copy_restricted: true,
            ..current
        },
    ];
    for prior in cases {
        assert_eq!(
            classify(Some(prior), /*mapping_can_represent*/ true),
            ContinuationInputMapping::NewTransport
        );
    }
}
