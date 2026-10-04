//! The Model Provider Adapter of the Contract-Faithful Fake Destination.

mod create_plan;
mod prose_changes;

use storyos_application::{
    CreateObservation, DestinationRequest, ModelProviderAdapter, ModelResponse, ModelStreamSink,
    ModelUsage, PreDispatchRefusal, PreparedRequest, StreamControl, WirePayloadProjection,
};
use storyos_core::HOST_FAKE_MAPPING_REVISION;

const HOST_FAKE_EXECUTION_PROFILE: &str = "storyos.host-fake.execution.v1";

/// Derives every observation from the request alone, so a restart loses nothing.
pub struct FakeDestination;

/// The single-use prepared exchange of one fake request.
pub struct FakeExchange(Option<create_plan::FakeCreate>);

impl ModelProviderAdapter for FakeDestination {
    type Prepared = FakeExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<FakeExchange>, PreDispatchRefusal> {
        let DestinationRequest::Create(create) = request;
        let projection = match &create.passage_input {
            Some(input) => {
                let mut wire = input.clone();
                wire["mapping_revision"] = serde_json::json!(HOST_FAKE_MAPPING_REVISION);
                let bytes = storyos_core::canonical_json(&wire);
                WirePayloadProjection {
                    execution_profile: HOST_FAKE_EXECUTION_PROFILE.to_owned(),
                    mapping_revision: HOST_FAKE_MAPPING_REVISION.to_owned(),
                    digest: format!("sha256:{}", storyos_core::hex_sha256(bytes.as_bytes())),
                    serialized_payload: Some(bytes),
                }
            }
            None => WirePayloadProjection {
                execution_profile: HOST_FAKE_EXECUTION_PROFILE.to_owned(),
                mapping_revision: HOST_FAKE_MAPPING_REVISION.to_owned(),
                digest: wire_digest(&create.author_message, &create.chapter_id),
                serialized_payload: None,
            },
        };
        Ok(PreparedRequest {
            projection,
            prepared: FakeExchange(
                (!create_plan::create_outcome_unknown(&create.author_message))
                    .then(|| create_plan::plan_create(create)),
            ),
        })
    }

    async fn exchange(
        &self,
        prepared: FakeExchange,
        sink: &mut impl ModelStreamSink,
    ) -> CreateObservation {
        let FakeExchange(Some(planned)) = prepared else {
            return CreateObservation::OutcomeUnknown {
                response_reference: None,
            };
        };
        if sink.append(&planned.items).await == StreamControl::Stop {
            return CreateObservation::OutcomeUnknown {
                response_reference: None,
            };
        }
        CreateObservation::Terminal(ModelResponse {
            items: planned.items,
            output: planned.output,
            usage: ModelUsage::Unknown,
            response_reference: None,
        })
    }
}

fn wire_digest(author_message: &str, chapter_id: &str) -> String {
    format!(
        "sha256:{}",
        storyos_core::hex_sha256(
            storyos_core::canonical_json(&serde_json::json!({
                "author_message": author_message,
                "chapter_id": chapter_id,
                "mapping_revision": HOST_FAKE_MAPPING_REVISION,
            }))
            .as_bytes(),
        )
    )
}
