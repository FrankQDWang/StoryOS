//! The Host Tool `hybrid_search_manuscript` (ADR 0095 sections 1 and 4): the fused result with
//! the vector of the question before a keyword search, the Project Scope, each settlement and
//! refusal of the question request, the author actions while the request is in progress, a
//! stopped Worker, the three facts of the Exposure, and a compaction. Test prefix leads `54`,
//! `b5`, `bb`, and `ba`.

use std::future::Future;
use std::pin::Pin;

use serde_json::{Value, json};
use storyos_adapter_fake_destination::{
    EMBEDDING_NOT_SUBMITTED_MARKER, EMBEDDING_REJECTION_MARKER, EMBEDDING_UNKNOWN_MARKER,
    FakeDestination, FakeEmbeddingExchange, FakeEmbeddings, fake_vector,
};
use storyos_application::{
    CompleteAgentRun, CompleteAgentRunError, ContractFaultPoint, ConversationSelection,
    EmbeddingObservation, EmbeddingProviderAdapter, EmbeddingRoute, NoContractFaults,
    NoEmbeddingDestination, NoToolDestination, PassageVectorStore, PauseAgentRunInput,
    PreDispatchRefusal, PreparedEmbeddingRequest, ProjectScope, QuestionVector, SteerAgentRunInput,
    complete_agent_run, complete_agent_run_with_destinations, rank_manuscript_passages,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, CallValidationRefusal,
    EmbeddingInput, EmbeddingRegistration, HybridSearchRefusal, IntakeRefusal,
    NO_SEARCH_BY_MEANING, PassageTerms,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_load_skill::tests::calls;
use crate::agent_run_read_chapter::tests::{
    boundary, call_ids, finish, no_public_call, outputs_record, sent,
};
use crate::agent_run_steering::tests::{cancellation, queued, step_requests};
use crate::agent_run_tool_recovery::tests::{Fault, FaultAt};
use crate::command_sequence::tests::{PAUSE_AGENT_RUN, STEER_AGENT_RUN, issued};
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::passage_vectors::tests::{Chapter, SHORE, chapter, index, prepend, texts};
use crate::project_embedding_binding::tests::{FAKE, embedding_project};

const QUESTION: &str = "谁在早上回到了港口？";

/// The store of a deployment with the fake embedding configuration.
fn configured(store: &PostgresProjectReader) -> PostgresProjectReader {
    store.clone().with_embedding_registration(FAKE)
}

/// The script of one hybrid search of `QUESTION` with the terms `terms`.
fn script(terms: &[&str]) -> String {
    let terms: String = terms.iter().map(|term| format!("|{term}")).collect();
    format!("SCRIPT:hybrid_search_manuscript {QUESTION}{terms}")
}

/// The current ranking of `terms` in `scope`: with the fake vector of `QUESTION`, or the
/// exact-term list of S7-C3a after the line that says that the search by meaning was not
/// available.
async fn ranked(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    terms: &[&str],
    vector: bool,
) -> String {
    let terms: Vec<String> = terms.iter().map(|term| (*term).to_owned()).collect();
    let terms = PassageTerms::new(&terms).unwrap();
    let values = fake_vector(QUESTION);
    let question = vector.then_some(QuestionVector {
        registration_revision: FAKE.unwrap().revision,
        values: &values,
    });
    let call_id = "call_hybrid_search_manuscript_0";
    let text = rank_manuscript_passages(store, scope, &terms, question, call_id)
        .await
        .unwrap()
        .unwrap()
        .text;
    match vector {
        true => text,
        false => format!("{NO_SEARCH_BY_MEANING}\n{text}"),
    }
}

/// The function names, the states, and the refusals of the ToolCalls of the Run.
async fn shape(admin: &Client, run_id: &str) -> Value {
    let calls = calls(admin, run_id).await;
    let calls = calls[1].as_array().unwrap().iter();
    calls
        .map(|call| json!([call[0], call[1], call[2]]))
        .collect()
}

/// The function names that the first decision request of the Run declares.
async fn declared(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT ((payload #>> '{wire,serialized_payload}')::jsonb -> 'tools')::text
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
              ORDER BY model_attempt_id LIMIT 1",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The settlement and the ToolCall of each Embedding Attempt of a question in the Project, in
/// the order of the attempts.
async fn question_attempts(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE(jsonb_agg(jsonb_build_array(settlement,
                      jsonb_path_query_first(attempt, '$.**.tool_call_id'))
                      ORDER BY embedding_attempt_id), '[]')::text
               FROM storyos.embedding_attempts
              WHERE project_id = $1::text::uuid AND cause = 'search_question'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The Tool names of a Run of a new Conversation, with the search by meaning or without it.
fn tools(hybrid: bool) -> Value {
    let names = [
        "propose_edit",
        "submit_research",
        "read_tool_result",
        "read_chapter",
        "search_manuscript",
        "hybrid_search_manuscript",
        "write_memory",
        "read_memory_entry",
        "search_memory",
        "finish",
    ];
    (names.into_iter())
        .filter(|name| hybrid || *name != "hybrid_search_manuscript")
        .collect()
}

/// A completed Run of the script `message` in a new Conversation of `chapter`.
async fn scripted(
    store: &PostgresProjectReader,
    admin: &Client,
    chapter: &Chapter,
    prefix: &str,
    message: &str,
) -> String {
    let new = ConversationSelection::New;
    let (scope, chapter_id) = (&chapter.scope, &chapter.chapter_id);
    let run_id = admitted(store, scope, chapter_id, prefix, message, new).await;
    complete(store, admin, scope, &run_id).await;
    run_id
}

/// One response has a hybrid search and then a keyword search. The Host executes them in call
/// order. The hybrid search sends one embedding request of its question, which names the ToolCall,
/// and it settles with the fused result of the Project of its Run, with no first line and no
/// Destination Attempt. A second Project has a passage nearer to the question, and the result does
/// not have it. The next request sends the result as a `manuscript_search_result` item, and the
/// public ToolCall list does not have it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_before_a_keyword_search_settles_with_the_fused_result_of_its_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let ours = chapter(&store, &admin, ["541", "542"], "543", &texts()).await;
    index(&store, &FakeEmbeddings, &ours).await;
    // A second Project holds the same term and a passage that is the question.
    let mut nearer = texts();
    nearer[1] = QUESTION.repeat(/*n*/ 20);
    let other = chapter(&store, &admin, ["544", "545"], "546", &nearer).await;
    index(&store, &FakeEmbeddings, &other).await;
    let message = format!("{}\nSCRIPT:search_manuscript 渔船", script(&["渔船"]));
    let run_id = scripted(&store, &admin, &ours, "547", &message).await;

    let text = ranked(&store, &ours.scope, &["渔船"], true).await;
    let calls = calls(&admin, &run_id).await;
    let ids = call_ids(&admin, &run_id, None).await;
    let output = json!([
        "call_hybrid_search_manuscript_0",
        boundary("call_hybrid_search_manuscript_0", "ok", &text)
    ]);
    assert_eq!(
        (
            declared(&admin, &run_id).await,
            calls[1][0].clone(),
            calls[1][1][0].clone(),
            text == ranked(&store, &ours.scope, &["渔船"], false).await,
            text.contains(&other.chapter_id) || text.contains(QUESTION),
            question_attempts(&admin, &ours.scope).await,
            question_attempts(&admin, &other.scope).await,
            sent(&admin, &run_id).await[1][2][0].clone(),
            outputs_record(&admin, &run_id, 1).await[1].clone(),
            no_public_call(&store, &admin, &ours.scope, &run_id, &ids[0]).await,
        ),
        (
            tools(true),
            json!(["hybrid_search_manuscript", "settled", null, text, true]),
            json!("search_manuscript"),
            false,
            false,
            json!([["vector", ids[0]]]),
            json!([]),
            output,
            json!([
                ["manuscript_search_result", ids[0], "exact_required"],
                ["manuscript_search_result", ids[1], "exact_required"],
            ]),
            true,
        )
    );
    assert_eq!(calls[1][2], finish());
}

/// The question request of each call is rejected, not submitted, or in OutcomeUnknown. Each call
/// settles with the exact-term list after the line that says that the search by meaning was not
/// available, and the Run completes.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_settled_request_with_no_vector_gives_the_exact_term_list() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let chapter = chapter(&store, &admin, ["bb1", "bb2"], "bb3", &texts()).await;
    index(&store, &FakeEmbeddings, &chapter).await;
    let message: Vec<String> = [
        EMBEDDING_REJECTION_MARKER,
        EMBEDDING_NOT_SUBMITTED_MARKER,
        EMBEDDING_UNKNOWN_MARKER,
    ]
    .iter()
    .map(|marker| format!("SCRIPT:hybrid_search_manuscript {marker} {QUESTION}|渔船"))
    .collect();
    let run_id = scripted(&store, &admin, &chapter, "bb4", &message.join("\n")).await;

    let text = ranked(&store, &chapter.scope, &["渔船"], false).await;
    let settled = json!(["hybrid_search_manuscript", "settled", null, text, true]);
    let ids = call_ids(&admin, &run_id, None).await;
    assert_eq!(
        (
            calls(&admin, &run_id).await,
            question_attempts(&admin, &chapter.scope).await,
        ),
        (
            json!([
                "completed",
                [settled.clone(), settled.clone(), settled, finish()]
            ]),
            json!([
                ["rejected", ids[0]],
                ["not_submitted", ids[1]],
                ["outcome_unknown", ids[2]],
            ]),
        )
    );
}

/// ADR 0091 section 3: an intake refusal of each field, and `hybrid_search_unavailable` when the
/// deployment has no embedding configuration at the call, or when the embedding request of the
/// question is refused before its claim. Each Run continues and completes, and no Embedding
/// Attempt is written.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refused_call_has_its_label_and_the_run_continues() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (plain, admin) = stores().await;
    let store = configured(&plain);
    let chapter = chapter(&store, &admin, ["548", "549"], "54a", &texts()).await;
    // An empty question, and an empty term.
    let message = format!(
        "SCRIPT:hybrid_search_manuscript |渔船\n{}",
        script(&["渔船", ""])
    );
    let intake = scripted(&store, &admin, &chapter, "54b", &message).await;
    // The request declares the Tool, and the Worker loses the Run after the response. The Worker
    // that takes the Run over has no embedding configuration.
    let new = ConversationSelection::New;
    let (scope, chapter_id) = (&chapter.scope, &chapter.chapter_id);
    let lost = admitted(&store, scope, chapter_id, "54c", &script(&[]), new).await;
    let observer = FaultAt::new(
        ContractFaultPoint::StreamCommitted,
        /*nth*/ 1,
        Fault::TakeOver {
            admin: &admin,
            run_id: lost.clone(),
        },
    );
    let claim = claim_run(&admin, scope.clone(), lost.clone()).await;
    let fake = (&FakeDestination, &RuntimeClock);
    let stale = complete_agent_run(&store, fake.0, &observer, fake.1, &claim).await;
    let claim = claim_run(&admin, scope.clone(), lost.clone()).await;
    complete_agent_run(&plain, fake.0, &NoContractFaults, fake.1, &claim)
        .await
        .unwrap();
    // The process has no embedding credential.
    let new = ConversationSelection::New;
    let no_credential = admitted(&store, scope, chapter_id, "bb5", &script(&[]), new).await;
    let claim = claim_run(&admin, scope.clone(), no_credential.clone()).await;
    let no_embeddings = (&NoToolDestination, &NoEmbeddingDestination);
    complete_agent_run_with_destinations(
        &store,
        fake.0,
        no_embeddings.0,
        no_embeddings.1,
        &NoContractFaults,
        fake.1,
        &claim,
    )
    .await
    .unwrap();

    let refused = |refusal: IntakeRefusal| {
        let refusal = CallValidationRefusal::Intake(refusal);
        json!([
            "hybrid_search_manuscript",
            "refused",
            refusal.label(),
            refusal.result_text(),
            true
        ])
    };
    let unavailable = HybridSearchRefusal::Unavailable;
    assert_eq!(
        (
            calls(&admin, &intake).await,
            matches!(stale, Err(CompleteAgentRunError::StaleFence)),
            declared(&admin, &lost).await,
            calls(&admin, &lost).await,
            calls(&admin, &no_credential).await == calls(&admin, &lost).await,
            question_attempts(&admin, scope).await,
        ),
        (
            json!([
                "completed",
                [
                    refused(IntakeRefusal::Query),
                    refused(IntakeRefusal::Terms),
                    finish()
                ]
            ]),
            true,
            tools(true),
            json!([
                "completed",
                [
                    [
                        "hybrid_search_manuscript",
                        "refused",
                        unavailable.label(),
                        unavailable.result().text,
                        true
                    ],
                    finish()
                ]
            ]),
            true,
            json!([]),
        )
    );
}

/// ADR 0095 section 4: with no embedding configuration of the deployment, or with no embedding
/// use binding of the Project, the request does not declare the Tool, and it declares each other
/// manuscript Tool. A call of the name is refused with `unexposed_name`, and the Run continues.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_tool_is_not_declared_with_no_configuration_or_no_binding() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (plain, admin) = stores().await;
    let store = configured(&plain);
    // The Project has the binding, and the deployment of the Worker has no configuration.
    let (bound, chapter_id, first) = embedding_project(&plain, &admin, ["54d", "54e"], FAKE).await;
    complete(&plain, &admin, &bound, &first).await;
    let new = ConversationSelection::New;
    let unconfigured = admitted(&plain, &bound, &chapter_id, "54f", &script(&[]), new).await;
    complete(&plain, &admin, &bound, &unconfigured).await;
    // The deployment has the configuration, and the Project has no binding.
    let (unbound, chapter_id, first) = queued(&store, "b51").await;
    complete(&store, &admin, &unbound, &first).await;
    let new = ConversationSelection::New;
    let no_binding = admitted(&store, &unbound, &chapter_id, "b52", &script(&[]), new).await;
    complete(&store, &admin, &unbound, &no_binding).await;

    let unexposed = CallValidationRefusal::Unexposed;
    let refused = json!([
        ["hybrid_search_manuscript", "refused", unexposed.label()],
        ["finish", "settled", null]
    ]);
    assert_eq!(
        (
            declared(&admin, &unconfigured).await,
            shape(&admin, &unconfigured).await,
            declared(&admin, &no_binding).await,
            shape(&admin, &no_binding).await,
            calls(&admin, &no_binding).await[0].clone(),
        ),
        (
            tools(false),
            refused.clone(),
            tools(false),
            refused,
            json!("completed"),
        )
    );
}

/// ADR 0095 section 4: the request declares the Tool only when the database has the vector
/// store. `verify-project-scope.sh` also runs this test on a database without pgvector, where
/// the call is refused with `unexposed_name`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_tool_follows_the_vector_store_of_the_database() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let chapter = chapter(&store, &admin, ["b53", "b54"], "b55", &texts()).await;
    let run_id = scripted(&store, &admin, &chapter, "b56", &script(&["渔船"])).await;
    let available = store.vector_store_available(&chapter.scope).await.unwrap();

    let (state, refusal) = match available {
        true => ("settled", Value::Null),
        false => ("refused", json!(CallValidationRefusal::Unexposed.label())),
    };
    assert_eq!(
        (
            declared(&admin, &run_id).await,
            shape(&admin, &run_id).await
        ),
        (
            tools(available),
            json!([
                ["hybrid_search_manuscript", state, refusal],
                ["finish", "settled", null]
            ]),
        )
    );
}

/// ADR 0093 section 5: the message has the compaction prefix of the fake before the script. The
/// summary request continues the response of the call, and the later request is a full input:
/// both send the exact result, as for a result of `search_manuscript`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_compaction_after_a_call_keeps_the_exact_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let chapter = chapter(&store, &admin, ["b57", "b58"], "b59", &texts()).await;
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. {}",
        script(&["灯塔", "渔船"])
    );
    let run_id = scripted(&store, &admin, &chapter, "b5a", &message).await;

    let text = ranked(&store, &chapter.scope, &["灯塔", "渔船"], true).await;
    let call_id = "call_hybrid_search_manuscript_0";
    let output = json!([[call_id, boundary(call_id, "ok", &text)]]);
    let sent = sent(&admin, &run_id).await;
    let ids = call_ids(&admin, &run_id, None).await;
    assert_eq!(
        (
            calls(&admin, &run_id).await[1][0][3].clone(),
            json!([sent[1][0], sent[1][2]]),
            json!([sent[2][0], sent[2][2], sent[2][3]]),
            outputs_record(&admin, &run_id, 1).await[1].clone(),
        ),
        (
            json!(text),
            json!(["compaction", output]),
            json!(["decision", output, true]),
            json!([["manuscript_search_result", ids[0], "exact_required"]]),
        )
    );
}

type Action<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// The fake embedding destination that runs one author action in the exchange of the first
/// question, while the Worker holds the Run, and then answers. With `stop`, the exchange never
/// answers, as for a stopped Worker.
struct Held<'a> {
    action: std::sync::Mutex<Option<Action<'a>>>,
    stop: bool,
}

impl<'a> Held<'a> {
    fn new(action: impl Future<Output = ()> + Send + 'a, stop: bool) -> Self {
        Self {
            action: std::sync::Mutex::new(Some(Box::pin(action))),
            stop,
        }
    }
}

impl EmbeddingProviderAdapter for Held<'_> {
    type Prepared = FakeEmbeddingExchange;

    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, PreDispatchRefusal> {
        FakeEmbeddings.prepare(registration, route, input).await
    }

    async fn exchange(&self, prepared: Self::Prepared) -> EmbeddingObservation {
        let action = self.action.lock().unwrap().take();
        if let Some(action) = action {
            action.await;
        }
        if self.stop {
            std::future::pending::<()>().await;
        }
        FakeEmbeddings.exchange(prepared).await
    }
}

/// Claims and completes the Run `run_id` with the embedding destination `held`.
async fn complete_held(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
    held: &Held<'_>,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    let (tools, faults) = (&NoToolDestination, &NoContractFaults);
    complete_agent_run_with_destinations(
        store,
        &FakeDestination,
        tools,
        held,
        faults,
        &RuntimeClock,
        &claim,
    )
    .await
}

async fn conversation(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// ADR 0095 section 1: the author acts while the question request is in progress. An Author Edit
/// does not wait, and the record ranks the current text, so the changed passage returns only from
/// the exact-term list. A steering input goes to the next RunStep with the result.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_edit_or_a_steering_input_during_the_question_request_does_not_wait() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let chapter = chapter(&store, &admin, ["bb6", "bb7"], "bb8", &texts()).await;
    index(&store, &FakeEmbeddings, &chapter).await;
    let (scope, chapter_id) = (&chapter.scope, &chapter.chapter_id);
    let message = script(&["渔船"]);
    let run = |prefix: &'static str| {
        let new = ConversationSelection::New;
        admitted(&store, scope, chapter_id, prefix, &message, new)
    };

    // An Author Edit puts the term before the second passage.
    let edited = run("bb9").await;
    let command = prepend(&store, &chapter, ("bba0", 2), 1, "渔船").await;
    let edit = async {
        store.apply_author_edit(&command).await.unwrap();
    };
    let edit = complete_held(&store, &admin, scope, &edited, &Held::new(edit, false)).await;
    let edited_text = ranked(&store, scope, &["渔船"], true).await;

    let steered = run("bbb").await;
    let input = SteerAgentRunInput {
        run_id: steered.clone(),
        conversation_id: conversation(&admin, &steered).await,
        author_message: "Keep the voice.".to_owned(),
        selection_references: None,
    };
    let steer = issued(&store, scope, 0xbbf0, &STEER_AGENT_RUN, input).await;
    let action = async {
        (store.steer_agent_run(&steer.envelope, &steer.input).await).unwrap();
    };
    let steer = complete_held(&store, &admin, scope, &steered, &Held::new(action, false)).await;

    let steps = step_requests(&admin, &steered).await;
    assert_eq!(
        (
            edit.is_ok(),
            calls(&admin, &edited).await[1][0][3].clone(),
            edited_text.contains(&format!("渔船{SHORE}")),
            steer.is_ok(),
            json!([steps[0][2], steps[1][2], steps[1][4]]),
        ),
        (
            true,
            json!(edited_text),
            true,
            true,
            json!([
                script(&["渔船"]),
                format!("{}\nKeep the voice.", script(&["渔船"])),
                ["call_hybrid_search_manuscript_0"]
            ]),
        )
    );
}

/// ADR 0095 section 1: a cancellation during the question request refuses the call with
/// `run_cancelled`, and a pause leaves the call unresolved. The later observation settles only
/// the Embedding Attempt, and no result enters the Run.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancellation_or_a_pause_during_the_question_request_keeps_the_result_out() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let chapter = chapter(&store, &admin, ["ba5", "ba6"], "ba7", &texts()).await;
    index(&store, &FakeEmbeddings, &chapter).await;
    let (scope, chapter_id) = (&chapter.scope, &chapter.chapter_id);
    let message = script(&["渔船"]);
    let run = |prefix: &'static str| {
        let new = ConversationSelection::New;
        admitted(&store, scope, chapter_id, prefix, &message, new)
    };

    let cancelled = run("ba8").await;
    let cancel = cancellation(&store, scope, 0xbaf0, &cancelled).await;
    let action = async {
        (store
            .cancel_agent_run(&cancel.envelope, &cancel.input)
            .await)
            .unwrap();
    };
    let cancel = complete_held(&store, &admin, scope, &cancelled, &Held::new(action, false)).await;

    let paused = run("ba9").await;
    let input = PauseAgentRunInput {
        run_id: paused.clone(),
    };
    let pause = issued(&store, scope, 0xbaf1, &PAUSE_AGENT_RUN, input).await;
    let action = async {
        (store.pause_agent_run(&pause.envelope, &pause.input).await).unwrap();
    };
    let pause = complete_held(&store, &admin, scope, &paused, &Held::new(action, false)).await;

    let ok_or_stale = |result: Result<CompleteAgentRun, CompleteAgentRunError>| {
        matches!(result, Ok(_) | Err(CompleteAgentRunError::StaleFence))
    };
    let mut attempts = Vec::new();
    for run_id in [&cancelled, &paused] {
        attempts.push(json!(["vector", call_ids(&admin, run_id, None).await[0]]));
    }
    assert_eq!(
        (
            ok_or_stale(cancel),
            shape(&admin, &cancelled).await,
            calls(&admin, &cancelled).await[0].clone(),
            ok_or_stale(pause),
            calls(&admin, &paused).await,
            question_attempts(&admin, scope).await,
        ),
        (
            true,
            json!([["hybrid_search_manuscript", "refused", "run_cancelled"]]),
            json!("cancelled"),
            true,
            json!([
                "paused",
                [["hybrid_search_manuscript", "pending", null, null, true]]
            ]),
            json!(attempts),
        )
    );
}

/// ADR 0095 section 3: the Worker stops while the question request is in progress. Its Embedding
/// Attempt stays in OutcomeUnknown with no settlement. The next claim of the Run sends a new
/// request with a new attempt, and it settles the call.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_claim_after_a_stopped_worker_sends_a_new_question_request() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let chapter = chapter(&store, &admin, ["ba1", "ba2"], "ba3", &texts()).await;
    index(&store, &FakeEmbeddings, &chapter).await;
    let (scope, chapter_id) = (&chapter.scope, &chapter.chapter_id);
    let new = ConversationSelection::New;
    let run_id = admitted(&store, scope, chapter_id, "ba4", &script(&["渔船"]), new).await;
    let (reached, reached_rx) = tokio::sync::oneshot::channel();
    let held = Held::new(
        async {
            let _reported = reached.send(());
        },
        /*stop*/ true,
    );
    tokio::select! {
        _ = complete_held(&store, &admin, scope, &run_id, &held) => {
            panic!("the held exchange does not answer")
        }
        _ = reached_rx => {}
    }
    let stopped = question_attempts(&admin, scope).await;
    complete(&store, &admin, scope, &run_id).await;

    let text = ranked(&store, scope, &["渔船"], true).await;
    let id = &call_ids(&admin, &run_id, None).await[0];
    assert_eq!(
        (
            stopped,
            question_attempts(&admin, scope).await,
            calls(&admin, &run_id).await,
        ),
        (
            json!([[null, id]]),
            json!([[null, id], ["vector", id]]),
            json!([
                "completed",
                [
                    ["hybrid_search_manuscript", "settled", null, text, true],
                    finish()
                ]
            ]),
        )
    );
}
