//! The Model Provider Adapter of the Contract-Faithful Fake Destination.

mod create_plan;
mod prose_changes;
mod recovery_plan;

use storyos_application::{
    DestinationRequest, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage,
    Observation, PreDispatchRefusal, PreparedRequest, ResponseReference, StreamControl,
    WirePayloadProjection,
};
use storyos_core::HOST_FAKE_MAPPING_REVISION;

const HOST_FAKE_EXECUTION_PROFILE: &str = "storyos.host-fake.execution.v1";

/// Derives every observation from the request alone, so a restart loses nothing.
pub struct FakeDestination;

/// The single-use prepared exchange of one fake request.
pub struct FakeExchange(FakePlan);

enum FakePlan {
    Create(create_plan::FakeCreate),
    UnknownCreate(Option<ResponseReference>),
    Retrieve(Observation),
}

impl ModelProviderAdapter for FakeDestination {
    type Prepared = FakeExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<FakeExchange>, PreDispatchRefusal> {
        let create = match request {
            DestinationRequest::Create(create) => create,
            DestinationRequest::Retrieve(retrieve) => {
                return Ok(PreparedRequest {
                    projection: projection(
                        wire_digest(&serde_json::json!({
                            "operation": "retrieve",
                            "response_reference": retrieve.response_reference,
                            "mapping_revision": HOST_FAKE_MAPPING_REVISION,
                        })),
                        /*serialized_payload*/ None,
                    ),
                    prepared: FakeExchange(FakePlan::Retrieve(recovery_plan::retrieve(retrieve))),
                });
            }
        };
        let projection = match &create.passage_input {
            Some(input) => {
                let mut wire = input.clone();
                wire["mapping_revision"] = serde_json::json!(HOST_FAKE_MAPPING_REVISION);
                let bytes = storyos_core::canonical_json(&wire);
                projection(
                    format!("sha256:{}", storyos_core::hex_sha256(bytes.as_bytes())),
                    Some(bytes),
                )
            }
            None => projection(
                wire_digest(&serde_json::json!({
                    "author_message": create.author_message,
                    "chapter_id": create.chapter_id,
                    "mapping_revision": HOST_FAKE_MAPPING_REVISION,
                })),
                /*serialized_payload*/ None,
            ),
        };
        let plan = match recovery_plan::unknown_create(&create.author_message, &projection.digest) {
            Some(reference) => FakePlan::UnknownCreate(reference),
            None => FakePlan::Create(create_plan::plan_create(create)),
        };
        Ok(PreparedRequest {
            projection,
            prepared: FakeExchange(plan),
        })
    }

    async fn exchange(
        &self,
        prepared: FakeExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let planned = match prepared.0 {
            FakePlan::Create(planned) => planned,
            FakePlan::UnknownCreate(response_reference) => {
                return Observation::OutcomeUnknown { response_reference };
            }
            FakePlan::Retrieve(observation) => return observation,
        };
        if sink.append(&planned.items).await == StreamControl::Stop {
            return Observation::OutcomeUnknown {
                response_reference: None,
            };
        }
        Observation::Terminal(ModelResponse {
            items: planned.items,
            output: planned.output,
            usage: ModelUsage::Unknown,
            response_reference: None,
        })
    }
}

fn projection(digest: String, serialized_payload: Option<String>) -> WirePayloadProjection {
    WirePayloadProjection {
        execution_profile: HOST_FAKE_EXECUTION_PROFILE.to_owned(),
        mapping_revision: HOST_FAKE_MAPPING_REVISION.to_owned(),
        digest,
        serialized_payload,
    }
}

fn wire_digest(wire: &serde_json::Value) -> String {
    format!(
        "sha256:{}",
        storyos_core::hex_sha256(storyos_core::canonical_json(wire).as_bytes())
    )
}
