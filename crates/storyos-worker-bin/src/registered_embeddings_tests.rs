use std::sync::Mutex;

use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_adapter_volcengine_responses::AgentPlanEmbeddings;
use storyos_application::{
    CredentialReference, CredentialResolver, EmbeddingProviderAdapter, EmbeddingRoute,
    PreDispatchRefusal, ResolvedCredential,
};
use storyos_core::{
    AGENT_PLAN_CREDENTIAL_REFERENCE, AGENT_PLAN_EMBEDDING_REGISTRATION, AGENT_PLAN_ENDPOINT,
    EmbeddingInput, EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION,
};

use super::{RegisteredEmbeddingExchange, RegisteredEmbeddings};

/// Records each resolved reference and returns a test value that is not a real credential.
struct RecordingResolver {
    available: bool,
    seen: Mutex<Vec<CredentialReference>>,
}

impl CredentialResolver for RecordingResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        self.seen.lock().unwrap().push(reference.clone());
        self.available
            .then(|| ResolvedCredential::new("storyos-credential-canary-3b8e".to_owned()))
    }
}

/// Which adapter prepared the request, or the refusal.
#[derive(Debug, Eq, PartialEq)]
enum Prepared {
    Fake,
    AgentPlan,
    Refused(PreDispatchRefusal),
}

async fn prepare(
    available: bool,
    registration: &'static EmbeddingRegistration,
    route: EmbeddingRoute,
) -> (Prepared, Vec<CredentialReference>) {
    let embeddings = RegisteredEmbeddings {
        fake: FakeEmbeddings,
        agent_plan: AgentPlanEmbeddings {
            resolver: RecordingResolver {
                available,
                seen: Mutex::default(),
            },
        },
    };
    let input = EmbeddingInput::Question {
        tool_call_id: "call-1".to_owned(),
        text: "Where is the lantern?".to_owned(),
    };
    let prepared = match embeddings.prepare(registration, &route, &input).await {
        Ok(prepared) => match prepared.prepared {
            RegisteredEmbeddingExchange::Fake(_) => Prepared::Fake,
            RegisteredEmbeddingExchange::AgentPlan(_) => Prepared::AgentPlan,
        },
        Err(refusal) => Prepared::Refused(refusal),
    };
    (
        prepared,
        embeddings.agent_plan.resolver.seen.into_inner().unwrap(),
    )
}

#[tokio::test]
async fn each_embedding_request_uses_the_adapter_of_its_registration_and_its_reference() {
    let reference = CredentialReference(AGENT_PLAN_CREDENTIAL_REFERENCE.to_owned());
    let agent_plan = EmbeddingRoute {
        registration_revision: AGENT_PLAN_EMBEDDING_REGISTRATION.revision.to_owned(),
        endpoint: Some(AGENT_PLAN_ENDPOINT.to_owned()),
        credential_reference: Some(reference.clone()),
    };
    let fake = EmbeddingRoute {
        registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision.to_owned(),
        endpoint: None,
        credential_reference: None,
    };

    assert_eq!(
        [
            prepare(
                /*available*/ true,
                &HOST_FAKE_EMBEDDING_REGISTRATION,
                fake.clone()
            )
            .await,
            prepare(
                /*available*/ true,
                &AGENT_PLAN_EMBEDDING_REGISTRATION,
                agent_plan.clone()
            )
            .await,
            prepare(
                /*available*/ false,
                &AGENT_PLAN_EMBEDDING_REGISTRATION,
                agent_plan
            )
            .await,
            prepare(
                /*available*/ true,
                &AGENT_PLAN_EMBEDDING_REGISTRATION,
                fake
            )
            .await,
        ],
        [
            (Prepared::Fake, vec![]),
            (Prepared::AgentPlan, vec![reference.clone()]),
            (
                Prepared::Refused(PreDispatchRefusal::CredentialUnavailable),
                vec![reference]
            ),
            (
                Prepared::Refused(PreDispatchRefusal::UnsupportedRequest),
                vec![]
            ),
        ]
    );
}

#[test]
fn the_worker_binary_claims_the_index_work_of_each_adapter_that_it_composes() {
    let registrations =
        <RegisteredEmbeddings<RecordingResolver> as EmbeddingProviderAdapter>::REGISTRATIONS;
    assert_eq!(
        registrations
            .iter()
            .map(|registration| registration.revision)
            .collect::<Vec<_>>(),
        [
            HOST_FAKE_EMBEDDING_REGISTRATION.revision,
            AGENT_PLAN_EMBEDDING_REGISTRATION.revision
        ]
    );
}
