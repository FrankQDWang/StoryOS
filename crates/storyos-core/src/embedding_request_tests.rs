use serde_json::json;

use super::*;

fn passage(text: &str) -> EmbeddingInput {
    EmbeddingInput::Passage {
        chapter_id: "ch-1".to_owned(),
        revision_id: "rev-1".to_owned(),
        passage_digest: "sha256:p".to_owned(),
        text: text.to_owned(),
    }
}

fn admitted<'a>(input: &'a EmbeddingInput) -> EmbeddingAdmissionFacts<'a> {
    EmbeddingAdmissionFacts {
        registration: Some(&HOST_FAKE_EMBEDDING_REGISTRATION),
        binding: Some(EmbeddingBindingFacts {
            registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision,
            grant_scope: Some("manuscript_embedding_v1"),
        }),
        source_in_scope: true,
        input,
    }
}

#[test]
fn the_admission_gives_one_typed_refusal_for_each_failed_fact() {
    let text = passage("The tide turned.");
    let question = EmbeddingInput::Question {
        tool_call_id: "call-1".to_owned(),
        text: "Where is the lantern?".to_owned(),
    };
    let empty = passage("");
    let at_limit = passage(&"潮".repeat(8_192));
    let above_limit = passage(&"潮".repeat(8_193));
    let research = EmbeddingBindingFacts {
        registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision,
        grant_scope: Some("research_tool_v1"),
    };
    let rows: [(EmbeddingAdmissionFacts<'_>, Result<(), EmbeddingRefusal>); 10] = [
        (admitted(&text), Ok(())),
        (admitted(&question), Ok(())),
        (admitted(&at_limit), Ok(())),
        (
            EmbeddingAdmissionFacts {
                registration: None,
                ..admitted(&text)
            },
            Err(EmbeddingRefusal::NotConfigured),
        ),
        (
            EmbeddingAdmissionFacts {
                binding: None,
                ..admitted(&text)
            },
            Err(EmbeddingRefusal::BindingAbsent),
        ),
        (
            EmbeddingAdmissionFacts {
                registration: Some(&AGENT_PLAN_EMBEDDING_REGISTRATION),
                ..admitted(&text)
            },
            Err(EmbeddingRefusal::BindingAbsent),
        ),
        (
            EmbeddingAdmissionFacts {
                binding: Some(research),
                ..admitted(&text)
            },
            Err(EmbeddingRefusal::DisclosureOutsideGrant),
        ),
        (
            EmbeddingAdmissionFacts {
                source_in_scope: false,
                ..admitted(&question)
            },
            Err(EmbeddingRefusal::DisclosureOutsideGrant),
        ),
        (admitted(&empty), Err(EmbeddingRefusal::InputOutOfBounds)),
        (
            admitted(&above_limit),
            Err(EmbeddingRefusal::InputOutOfBounds),
        ),
    ];

    for (facts, expected) in rows {
        assert_eq!(admit_embedding(&facts), expected);
    }
    assert_eq!(
        [
            EmbeddingRefusal::NotConfigured,
            EmbeddingRefusal::BindingAbsent,
            EmbeddingRefusal::DisclosureOutsideGrant,
            EmbeddingRefusal::InputOutOfBounds,
            EmbeddingRefusal::CredentialUnavailable,
        ]
        .map(EmbeddingRefusal::code),
        [
            "embedding_not_configured",
            "embedding_binding_absent",
            "disclosure_outside_grant",
            "embedding_input_out_of_bounds",
            "destination_credential_unavailable",
        ]
    );
}

#[test]
fn only_the_embedding_scope_names_an_embedding_request_and_it_names_no_model_request() {
    let scopes = [
        "project_writing_assistance_v1",
        "project_writing_assistance_v2",
        "project_writing_assistance_v3",
        "project_writing_assistance_v4",
        "project_writing_assistance_v5",
        "research_tool_v1",
        "manuscript_embedding_v1",
    ]
    .map(|name| GrantScope::parse(name).unwrap());

    let names = scopes.map(|scope| {
        (
            scope.as_str(),
            scope.names_embedding(EmbeddingCause::PassageIndex),
            scope.names_embedding(EmbeddingCause::SearchQuestion),
        )
    });

    assert_eq!(
        names,
        [
            ("project_writing_assistance_v1", false, false),
            ("project_writing_assistance_v2", false, false),
            ("project_writing_assistance_v3", false, false),
            ("project_writing_assistance_v4", false, false),
            ("project_writing_assistance_v5", false, false),
            ("research_tool_v1", false, false),
            ("manuscript_embedding_v1", true, true),
        ]
    );
    let grant = crate::GrantFacts {
        owner_user_id: "user-1",
        project_id: "proj-1",
        grant_scope: Some("manuscript_embedding_v1"),
        grant_identity: Some("dest-1"),
        binding_identity: Some("dest-1"),
        request: None,
    };
    assert_eq!(
        crate::admit_disclosure(&grant),
        Err("disclosure_outside_grant")
    );
}

#[test]
fn each_cause_has_its_source_class_and_its_own_instructions() {
    let question = EmbeddingInput::Question {
        tool_call_id: "call-1".to_owned(),
        text: "Where is the lantern?".to_owned(),
    };
    let causes = [passage("The tide turned.").cause(), question.cause()];

    assert_eq!(
        causes.map(|cause| (cause.as_str(), cause.source_class())),
        [
            ("passage_index", "chapter_passage"),
            ("search_question", "search_question")
        ]
    );
    assert_eq!(MANUSCRIPT_EMBEDDING_PURPOSE, "manuscript_embedding");
    assert_ne!(causes[0].instructions(), causes[1].instructions());
}

#[test]
fn each_destination_has_one_text_registration_of_1024_dimensions() {
    let registrations = [
        DestinationKind::HostFake,
        DestinationKind::VolcengineAgentPlan,
    ]
    .map(EmbeddingRegistration::of);

    assert_eq!(
        registrations.map(|registration| (
            registration.destination,
            registration.adapter.as_str(),
            registration.provider_model_id,
            registration.dimensions,
            registration.input_modality,
            registration.request_bounds,
        )),
        [
            (
                DestinationKind::HostFake,
                "host_fake",
                "host-fake-embedding",
                1_024,
                "text",
                "embedding_request_v1"
            ),
            (
                DestinationKind::VolcengineAgentPlan,
                "volcengine_agent_plan_embeddings",
                "doubao-embedding-vision-251215",
                1_024,
                "text",
                "embedding_request_v1"
            ),
        ]
    );
    assert_eq!(EMBEDDING_REQUEST_TIMEOUT.as_secs(), 30);
}

#[test]
fn the_snapshot_names_the_cause_and_the_references_and_no_text() {
    let question = EmbeddingInput::Question {
        tool_call_id: "call-1".to_owned(),
        text: "Where is the lantern?".to_owned(),
    };

    assert_eq!(
        [passage("The tide turned.").snapshot(), question.snapshot()],
        [
            json!({
                "cause": "passage_index", "purpose": "manuscript_embedding",
                "source_class": "chapter_passage",
                "texts": [{"chapter_id": "ch-1", "revision_id": "rev-1",
                           "passage_digest": "sha256:p"}],
                "working_target": "not_applicable", "workspace_context": "not_applicable",
            }),
            json!({
                "cause": "search_question", "purpose": "manuscript_embedding",
                "source_class": "search_question", "texts": [{"tool_call_id": "call-1"}],
                "working_target": "not_applicable", "workspace_context": "not_applicable",
            }),
        ]
    );
    assert_ne!(
        EmbeddingCause::PassageIndex.instructions(),
        EmbeddingCause::SearchQuestion.instructions()
    );
}
