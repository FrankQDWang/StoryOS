use super::{
    ContinuationIdentity, ContinuationInputMapping, ContinuationMappingInput,
    map_continuation_input,
};
use crate::{HOST_FAKE_MAPPING_REVISION, ToolExposure};

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
        tool_exposure_digest: ToolExposure::empty().digest(),
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
        prior_reference_unusable: false,
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
            tool_exposure_digest: "sha256:changed-exposure".to_owned(),
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

#[test]
fn a_changed_exposure_changes_the_transport_and_not_the_processing_boundary() {
    let current = matching_prior();
    let prior = ContinuationIdentity {
        tool_exposure_digest: "sha256:changed-exposure".to_owned(),
        ..current.clone()
    };

    assert!(super::continuation_boundary_matches(&current, &prior));
}

#[test]
fn an_unusable_prior_reference_sends_the_full_input() {
    let input = |prior_reference_unusable| ContinuationMappingInput {
        current: matching_prior(),
        prior: Some(matching_prior()),
        mapping_can_represent: true,
        prior_reference_unusable,
    };

    assert_eq!(
        [
            map_continuation_input(&input(false)),
            map_continuation_input(&input(true)),
        ],
        [
            ContinuationInputMapping::Incremental,
            ContinuationInputMapping::Full,
        ]
    );
}
