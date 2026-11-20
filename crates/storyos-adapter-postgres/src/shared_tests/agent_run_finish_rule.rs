//! The shared test support from `agent_run_finish_rule_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::model_gateway::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering,
};
use serde_json::{Value, json};
use storyos_adapter_fake_destination::{FakeDestination, FakeEmbeddings};
use storyos_application::{
    ConversationSelection, ModelResponse, NoContractFaults, NoToolDestination, ProjectScope,
    complete_agent_run_with_destinations,
};
use storyos_core::{
    DeploymentDestination, NativeStreamItem, ResearchToolOffer, StreamItemRole, StreamItemState,
};
use tokio_postgres::Client;

/// Claims and completes the queued Run `run_id` in a process with no Tool adapter and with the
/// fake embedding destination.
pub(crate) async fn complete(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run_with_destinations(
        store,
        &FakeDestination,
        &NoToolDestination,
        &FakeEmbeddings,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
}

/// Completes the first Run of a new Project of the tool loop profile, then completes one Run with
/// `author_message` in a new Conversation or in the Conversation of the first Run. `prefixes`
/// give the identities of the two Runs.
pub(crate) async fn tool_loop_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    author_message: &str,
    research: Option<&'static ResearchToolOffer>,
    same_conversation: bool,
) -> (ProjectScope, String, String, String) {
    let destination = DeploymentDestination::HostFakeToolLoop;
    let (scope, chapter_id, first) =
        queued_run_offering(store, prefixes[0], destination.clone(), research).await;
    complete(store, admin, &scope, &first).await;
    let conversation = match same_conversation {
        true => conversation_of(prefixes[0]),
        false => ConversationSelection::New,
    };
    let run_id = admit_run_in(
        store,
        &scope,
        &chapter_id,
        prefixes[1],
        destination,
        author_message,
        conversation,
        research,
    )
    .await;
    complete(store, admin, &scope, &run_id).await;
    (scope, chapter_id, first, run_id)
}

/// One complete assistant message of `text`.
pub(crate) fn text_item(text: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "msg_01".to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Complete,
        text: Some(text.to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

/// One complete `finish` call with `call_id`.
pub(crate) fn finish_item(call_id: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: format!("fc_{call_id}"),
        role: StreamItemRole::Tool,
        text: None,
        call_id: Some(call_id.to_owned()),
        arguments: Some("{}".to_owned()),
        native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some("finish".to_owned()),
        ..text_item("")
    }
}

/// One complete `propose_edit` call with `call_id` that proposes `changes`.
pub(crate) fn propose_edit_item(
    call_id: &str,
    changes: &[storyos_core::ProseChangeCandidate],
) -> NativeStreamItem {
    let changes: Vec<Value> = changes
        .iter()
        .map(|change| {
            json!({"block_id": change.manuscript_block_id,
                   "candidate_text": change.candidate_text, "explanation": change.explanation})
        })
        .collect();
    NativeStreamItem {
        arguments: Some(json!({"summary": "Shorter.", "changes": changes}).to_string()),
        name: Some("propose_edit".to_owned()),
        ..finish_item(call_id)
    }
}

/// The response that a model of the tool loop gives before its closing `response`: one
/// `finish` call with the usage of `response`. Its reference is the reference of `response`
/// with `_finish`.
pub(crate) fn finish_before(response: &ModelResponse) -> ModelResponse {
    ModelResponse {
        state: StreamItemState::Complete,
        items: vec![finish_item("call_finish")],
        response_reference: response
            .response_reference
            .as_ref()
            .map(|reference| format!("{reference}_finish")),
        ..response.clone()
    }
}

/// The Conversation of the first Run of the Project of `prefix`.
pub(crate) fn conversation_of(prefix: &str) -> ConversationSelection {
    ConversationSelection::Existing {
        conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
    }
}

/// For each decision Model Attempt of the Run in RunStep order: the decision kind, the
/// `tool_choice` of its request, its response reference, and the prior response reference that
/// its request sent.
pub(crate) async fn steps(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(payload #>> '{decision,kind}', wire ->> 'tool_choice',
                                      payload ->> 'response_reference',
                                      wire ->> 'previous_response_reference')::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(0)).unwrap())
        .collect()
}

/// Each ToolCall of the Run in RunStep and item order: the RunStep, the function name, the
/// state, the refusal, and the result text.
pub(crate) async fn calls(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(decision_position, function_name, state, refusal,
                                      result_text)::text
               FROM storyos.tool_calls WHERE run_id = $1::text::uuid
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(0)).unwrap())
        .collect()
}
