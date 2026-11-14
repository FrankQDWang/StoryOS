//! The Model Provider Adapter of the Contract-Faithful Fake Destination.

mod create_plan;
mod propose_edit_plan;
mod prose_changes;
mod read_plan;
mod recovery_plan;
mod research_plan;
mod tool_loop_plan;

pub use tool_loop_plan::LIMIT_ANSWER;

use storyos_application::{
    CacheUse, CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter,
    ModelResponse, ModelStreamSink, ModelUsage, Observation, PreDispatchRefusal, PreparedRequest,
    PriorInput, ResponseReference, StreamControl, WirePayloadProjection,
};
use storyos_core::{
    ContextSourceClass, DeclaredTargetKind, HOST_FAKE_MAPPING_REVISION,
    HOST_FAKE_TOOL_LOOP_MAPPING_REVISION, ModelAdapter, StreamItemState,
};

const HOST_FAKE_EXECUTION_PROFILE: &str = "storyos.host-fake.execution.v1";

/// Derives every observation from the request alone, so a restart loses nothing.
pub struct FakeDestination;

/// The single-use prepared exchange of one fake request.
pub struct FakeExchange(FakePlan);

enum FakePlan {
    Create(create_plan::FakeCreate, String),
    UnknownCreate(Option<ResponseReference>),
    Observed(Observation),
}

impl ModelProviderAdapter for FakeDestination {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::HostFake];
    type Prepared = FakeExchange;

    #[tracing::instrument(skip_all, level = "debug")]
    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<FakeExchange>, PreDispatchRefusal> {
        let create = match request {
            DestinationRequest::Create(create) => create,
            DestinationRequest::Abort(abort) => {
                return Ok(PreparedRequest {
                    projection: projection(
                        wire_digest(&serde_json::json!({
                            "operation": "abort",
                            "model_attempt_id": abort.ticket.model_attempt_id(),
                            "response_reference": abort.response_reference,
                            "mapping_revision": HOST_FAKE_MAPPING_REVISION,
                        })),
                        /*serialized_payload*/ None,
                    ),
                    prepared: FakeExchange(FakePlan::Observed(Observation::Terminal(
                        ModelResponse {
                            state: storyos_core::StreamItemState::Complete,
                            items: Vec::new(),
                            usage: ModelUsage::Unknown,
                            cache_use: CacheUse::Unknown,
                            response_reference: abort.response_reference.clone(),
                        },
                    ))),
                });
            }
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
                    prepared: FakeExchange(FakePlan::Observed(recovery_plan::retrieve(retrieve))),
                });
            }
        };
        if create.purpose == CreatePurpose::CompactionSummary {
            return Ok(summary(create));
        }
        // A decision request of the earlier mapping held the reply form (ADR 0091).
        if create.route.mapping_revision != HOST_FAKE_TOOL_LOOP_MAPPING_REVISION {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        let mapping_revision = HOST_FAKE_TOOL_LOOP_MAPPING_REVISION;
        let mut wire = current_input(create);
        wire["mapping_revision"] = serde_json::json!(mapping_revision);
        if let Some(reference) = &create.previous_response_reference {
            wire["previous_response_reference"] = serde_json::json!(reference);
        }
        wire["tool_choice"] = serde_json::json!(create.tool_choice.as_str());
        wire["tools"] = create
            .tool_exposure
            .tools
            .iter()
            .map(|tool| tool.name)
            .collect();
        wire["tool_outputs"] = create
            .tool_outputs
            .iter()
            .map(|output| serde_json::json!({"call_id": output.call_id, "output": output.output}))
            .collect();
        if !create.unexecuted_call_ids.is_empty() {
            wire["unexecuted_call_ids"] = serde_json::json!(create.unexecuted_call_ids);
        }
        let bytes = storyos_core::canonical_json(&wire);
        let mut projection = projection(
            format!("sha256:{}", storyos_core::hex_sha256(bytes.as_bytes())),
            Some(bytes),
        );
        projection.mapping_revision = mapping_revision.to_owned();
        // A continuation rejection applies only to the first RunStep of a Run, and a RunStep that
        // continues from a recovered response is not lost again.
        let later_step = !create.tool_outputs.is_empty();
        let rejection = create
            .previous_response_reference
            .as_deref()
            .filter(|_| !later_step)
            .and_then(recovery_plan::continuation_rejection);
        let after_recovery = create
            .previous_response_reference
            .as_deref()
            .is_some_and(recovery_plan::follows_recovery);
        let unknown = match create.successor_of {
            Some(_) => None,
            None if after_recovery => None,
            None => recovery_plan::unknown_create(
                &create.author_message,
                &projection.digest,
                mapping_revision,
            ),
        };
        let unanswered = recovery_plan::leaves_prior_calls_unanswered(create)
            .then_some(recovery_plan::UNANSWERED_CALL_REASON)
            .or_else(|| stub_names_unsent_call(create).then_some(STUB_CALL_MISSING_REASON));
        let plan = match (unanswered.or(rejection), unknown) {
            (Some(reason), _) => FakePlan::Observed(Observation::Rejected {
                reason: reason.to_owned(),
            }),
            (None, Some(reference)) => FakePlan::UnknownCreate(reference),
            (None, None) => {
                let planned = tool_loop_plan::plan(create);
                let reference = recovery_plan::mint_response_reference(
                    &projection.digest,
                    match create.successor_of {
                        Some(_) => recovery_plan::SUCCESSOR_RESPONSE,
                        None => recovery_plan::continuation_code(&create.author_message),
                    },
                    &planned.items,
                );
                FakePlan::Create(planned, reference)
            }
        };
        Ok(PreparedRequest {
            projection,
            prepared: FakeExchange(plan),
        })
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn exchange(
        &self,
        prepared: FakeExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let (planned, reference) = match prepared.0 {
            FakePlan::Create(planned, reference) => (planned, reference),
            FakePlan::UnknownCreate(response_reference) => {
                return Observation::OutcomeUnknown { response_reference };
            }
            FakePlan::Observed(observation) => return observation,
        };
        if sink.append(&planned.items).await == StreamControl::Stop {
            return Observation::OutcomeUnknown {
                response_reference: None,
            };
        }
        Observation::Terminal(ModelResponse {
            state: storyos_core::StreamItemState::Complete,
            items: planned.items,
            usage: ModelUsage::Unknown,
            cache_use: CacheUse::Unknown,
            response_reference: Some(reference),
        })
    }
}

/// The author message after the compaction prefix of the Host fake and the sentence that names
/// its RunStep positions, if any.
fn script_message(message: &str) -> &str {
    let Some(rest) = message.strip_prefix(storyos_core::ACTIVE_COMPACTION_REQUEST_PREFIX) else {
        return message;
    };
    let rest = rest.trim_start();
    match rest.strip_prefix(storyos_core::ACTIVE_COMPACTION_POSITIONS_PREFIX) {
        Some(named) => named
            .split_once('.')
            .map_or("", |(_, after)| after.trim_start()),
        None => rest,
    }
}

/// The reason of the rejection of a request with a stub that names a call that the request does
/// not send (#1507).
const STUB_CALL_MISSING_REASON: &str = "stub_call_missing";

/// Whether an output that `create` sends is a stub that names a call that the request does not
/// send: as its own call, or as the call that a read stub read (ADR 0093 section 5). A full input
/// sends the call of each output. A request with a prior response reference sends only the
/// outputs of the calls of that response.
fn stub_names_unsent_call(create: &CreateRequest) -> bool {
    let sent: Vec<&storyos_application::ToolOutput> = create
        .tool_outputs
        .iter()
        .filter(|output| {
            create.previous_response_reference.is_none()
                || output.origin == storyos_application::ToolOutputOrigin::PriorResponse
        })
        .collect();
    let named = |text: &str, marker: &str| {
        text.rsplit_once(marker)
            .and_then(|(_, rest)| rest.split(['"', '.']).next())
            .map(str::to_owned)
    };
    sent.iter()
        .filter(|output| output.output.contains("form=\"stub\""))
        .any(|output| {
            let own = named(&output.output, "<tool_result source=\"");
            let read = named(&output.output, "with the call_id ");
            own.as_deref() != Some(output.call_id.as_str())
                || !read.is_some_and(|read| sent.iter().any(|call| call.call_id == read))
        })
}

/// The current input that a request sends, as the tool loop wire sends it: an incremental
/// continuation with Tool outputs sends it only when it changed after the prior response read it.
/// A changed input says that it replaces the earlier input (ADR 0085).
fn current_input(create: &CreateRequest) -> serde_json::Value {
    let incremental =
        create.previous_response_reference.is_some() && !create.tool_outputs.is_empty();
    let replaced =
        create.previous_response_reference.is_some() && create.prior_input == PriorInput::Replaced;
    if incremental && !replaced {
        return serde_json::json!({});
    }
    let mut wire = match &create.passage_input {
        Some(input) => input.clone(),
        None => serde_json::json!({
            "author_message": create.author_message,
            "chapter_id": create.chapter_id,
        }),
    };
    wire["context"] = create
        .context
        .iter()
        .filter(|item| {
            !matches!(
                item.source_class,
                ContextSourceClass::HostControl | ContextSourceClass::ToolResult
            )
        })
        .map(|item| {
            serde_json::json!({"source_class": item.source_class.name(), "content": item.content})
        })
        .collect();
    wire["declared_targets"] = create
        .declared_targets
        .iter()
        .map(|target| {
            let kind = match target.kind {
                DeclaredTargetKind::ChapterBlock => "chapter_block",
                DeclaredTargetKind::PendingCandidate => "pending_candidate",
                DeclaredTargetKind::JoinableBlock { .. } => "joinable_block",
            };
            serde_json::json!({"block_id": target.block_id, "kind": kind})
        })
        .collect();
    if replaced {
        wire["replaces_earlier_input"] = serde_json::json!(true);
    }
    wire
}

/// The fixed Host-managed summary of the fake destination, as one complete assistant item.
fn summary(create: &CreateRequest) -> PreparedRequest<FakeExchange> {
    let mut wire = serde_json::json!({
        "operation": "compaction_summary",
        "previous_response_reference": create.previous_response_reference,
        "mapping_revision": HOST_FAKE_MAPPING_REVISION,
    });
    // The outputs that answer the calls of the prior response, as the tool loop wire sends them.
    let answers = !create.tool_outputs.is_empty() || !create.unexecuted_call_ids.is_empty();
    let serialized_payload = answers.then(|| {
        wire["tool_outputs"] = create
            .tool_outputs
            .iter()
            .map(|output| serde_json::json!({"call_id": output.call_id, "output": output.output}))
            .collect();
        if !create.unexecuted_call_ids.is_empty() {
            wire["unexecuted_call_ids"] = serde_json::json!(create.unexecuted_call_ids);
        }
        storyos_core::canonical_json(&wire)
    });
    let digest = wire_digest(&wire);
    if recovery_plan::leaves_prior_calls_unanswered(create) {
        return PreparedRequest {
            projection: projection(digest, serialized_payload),
            prepared: FakeExchange(FakePlan::Observed(Observation::Rejected {
                reason: recovery_plan::UNANSWERED_CALL_REASON.to_owned(),
            })),
        };
    }
    let item = create_plan::assistant_item(
        "summary",
        StreamItemState::Complete,
        storyos_core::HOST_FAKE_COMPACTION_OUTPUT,
    );
    let reference = recovery_plan::mint_response_reference(&digest, /*code*/ 0, &[]);
    PreparedRequest {
        projection: projection(digest, serialized_payload),
        prepared: FakeExchange(FakePlan::Observed(Observation::Terminal(ModelResponse {
            state: storyos_core::StreamItemState::Complete,
            items: vec![item],
            usage: ModelUsage::Unknown,
            cache_use: CacheUse::Unknown,
            response_reference: Some(reference),
        }))),
    }
}

fn projection(digest: String, serialized_payload: Option<String>) -> WirePayloadProjection {
    WirePayloadProjection {
        execution_profile: HOST_FAKE_EXECUTION_PROFILE.to_owned(),
        mapping_revision: HOST_FAKE_MAPPING_REVISION.to_owned(),
        digest,
        serialized_payload,
        cache_setting: None,
    }
}

fn wire_digest(wire: &serde_json::Value) -> String {
    format!(
        "sha256:{}",
        storyos_core::hex_sha256(storyos_core::canonical_json(wire).as_bytes())
    )
}

#[cfg(test)]
#[path = "stub_rejection_tests.rs"]
mod stub_rejection_tests;
