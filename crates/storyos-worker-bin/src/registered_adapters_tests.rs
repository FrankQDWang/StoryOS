use std::sync::Mutex;

use storyos_adapter_fake_destination::FakeDestination;
use storyos_adapter_volcengine_responses::AgentPlanResponses;
use storyos_application::{
    CreateRequest, CredentialReference, CredentialResolver, DestinationRequest,
    ModelProviderAdapter, PreDispatchRefusal, RequestAttempt, RequestBounds, RequestRoute,
    ResolvedCredential,
};
use storyos_core::ModelAdapter;

use super::RegisteredAdapters;

const REFERENCE: &str = "macos-keychain:storyos-volcengine-agent-plan/frankqdwang";
const CREDENTIAL_CANARY: &str = "storyos-credential-canary-7f3c";

/// Records each resolved reference and returns a test value that is not a real credential.
struct RecordingResolver {
    available: bool,
    seen: Mutex<Vec<CredentialReference>>,
}

impl CredentialResolver for RecordingResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        self.seen.lock().unwrap().push(reference.clone());
        self.available
            .then(|| ResolvedCredential::new(CREDENTIAL_CANARY.to_owned()))
    }
}

fn create(adapter: ModelAdapter, reference: Option<&str>) -> DestinationRequest {
    DestinationRequest::Create(CreateRequest {
        attempt: RequestAttempt::New,
        route: RequestRoute {
            adapter,
            provider_model_id: "doubao-seed-2.1-pro".to_owned(),
            endpoint: Some(storyos_core::AGENT_PLAN_ENDPOINT.to_owned()),
            credential_reference: reference.map(|value| CredentialReference(value.to_owned())),
            bounds: Some(RequestBounds {
                max_output_tokens: 8192,
                timeout: std::time::Duration::from_secs(180),
            }),
        },
        author_message: "Tighten this paragraph.".to_owned(),
        chapter_id: "018f0000-0000-7001-8000-0000000000c1".to_owned(),
        passage_resolution: None,
        passage_input: None,
        context: Vec::new(),
        declared_targets: Vec::new(),
        candidate_revision: None,
        previous_response_reference: None,
        successor_of: None,
    })
}

async fn prepare(
    available: bool,
    request: DestinationRequest,
) -> (Result<(), PreDispatchRefusal>, Vec<CredentialReference>) {
    let adapters = RegisteredAdapters {
        fake: FakeDestination,
        agent_plan: AgentPlanResponses {
            resolver: RecordingResolver {
                available,
                seen: Mutex::default(),
            },
        },
    };
    let prepared = adapters.prepare(&request).await.map(|_| ());
    (
        prepared,
        adapters.agent_plan.resolver.seen.into_inner().unwrap(),
    )
}

#[tokio::test]
async fn each_request_uses_the_adapter_of_its_registration_and_its_credential_reference() {
    let reference = CredentialReference(REFERENCE.to_owned());

    assert_eq!(
        [
            prepare(
                /*available*/ true,
                create(ModelAdapter::HostFake, None)
            )
            .await,
            prepare(
                /*available*/ false,
                create(ModelAdapter::VolcengineAgentPlanResponses, Some(REFERENCE))
            )
            .await,
            prepare(
                /*available*/ true,
                create(ModelAdapter::VolcengineAgentPlanResponses, None)
            )
            .await,
            prepare(
                /*available*/ true,
                create(ModelAdapter::VolcengineAgentPlanResponses, Some(REFERENCE))
            )
            .await,
        ],
        [
            (Ok(()), vec![]),
            (
                Err(PreDispatchRefusal::CredentialUnavailable),
                vec![reference.clone()]
            ),
            (Err(PreDispatchRefusal::CredentialUnavailable), vec![]),
            (Ok(()), vec![reference]),
        ]
    );
}

/// Collects the JSON lines of the Diagnostic Projection in memory.
#[derive(Clone, Default)]
struct Projection(std::sync::Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Projection {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn a_resolved_credential_never_reaches_the_diagnostic_projection() {
    let projection = Projection::default();
    let writer = projection.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::DEBUG)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_writer(move || writer.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);

    let prepared = prepare(
        /*available*/ true,
        create(ModelAdapter::VolcengineAgentPlanResponses, Some(REFERENCE)),
    )
    .await;

    let lines = String::from_utf8(projection.0.lock().unwrap().clone()).unwrap();
    assert_eq!(
        (
            prepared.0,
            lines.contains(r#""name":"prepare""#),
            lines.contains(CREDENTIAL_CANARY),
        ),
        (Ok(()), true, false)
    );
}
