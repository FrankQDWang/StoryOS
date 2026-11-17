//! The Host Tool `hybrid_search_manuscript` (ADR 0095 sections 1 and 4): the settled call beside a
//! keyword search with no embedding request, the Project Scope, each refusal, the three facts of
//! the Exposure, and a compaction. Test prefix leads `54` and `b5`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    CompleteAgentRunError, ContractFaultPoint, ConversationSelection, NoContractFaults,
    PassageVectorStore, ProjectScope, complete_agent_run, rank_manuscript_passages,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, CallValidationRefusal,
    HybridSearchRefusal, IntakeRefusal, NO_SEARCH_BY_MEANING, PassageTerms,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_load_skill::tests::calls;
use crate::agent_run_read_chapter::tests::{
    boundary, call_ids, finish, no_public_call, outputs_record, sent,
};
use crate::agent_run_steering::tests::queued;
use crate::agent_run_tool_recovery::tests::{Fault, FaultAt};
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::passage_vectors::tests::{Chapter, chapter, texts};
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

/// The exact-term list of S7-C3a for `terms` in `scope`, after the line that says that the search
/// by meaning was not available.
async fn exact_list(store: &PostgresProjectReader, scope: &ProjectScope, terms: &[&str]) -> String {
    let terms: Vec<String> = terms.iter().map(|term| (*term).to_owned()).collect();
    let terms = PassageTerms::new(&terms).unwrap();
    let call_id = "call_hybrid_search_manuscript_0";
    let text = rank_manuscript_passages(store, scope, &terms, None, call_id)
        .await
        .unwrap()
        .unwrap()
        .text;
    format!("{NO_SEARCH_BY_MEANING}\n{text}")
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

/// The count of the Embedding Attempts of a question in the Project.
async fn question_attempts(admin: &Client, scope: &ProjectScope) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.embedding_attempts
              WHERE project_id = $1::text::uuid AND cause = 'search_question'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
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

/// One response has a keyword search and a hybrid search. The Host executes them in call order.
/// The hybrid search settles with the exact-term list of the Project of its Run, after one line,
/// and it writes no Embedding Attempt and no Destination Attempt. The next request sends the
/// result as a `manuscript_search_result` item, and the public ToolCall list does not have it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_beside_a_keyword_search_settles_with_the_exact_term_list_of_its_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let store = configured(&store);
    let ours = chapter(&store, &admin, ["541", "542"], "543", &texts()).await;
    // A second Project holds the same term.
    let other = chapter(&store, &admin, ["544", "545"], "546", &texts()).await;
    let message = format!("SCRIPT:search_manuscript 渔船\n{}", script(&["渔船"]));
    let run_id = scripted(&store, &admin, &ours, "547", &message).await;

    let text = exact_list(&store, &ours.scope, &["渔船"]).await;
    let calls = calls(&admin, &run_id).await;
    let ids = call_ids(&admin, &run_id, None).await;
    let output = json!([
        "call_hybrid_search_manuscript_1",
        boundary("call_hybrid_search_manuscript_1", "ok", &text)
    ]);
    assert_eq!(
        (
            declared(&admin, &run_id).await,
            calls[1][1].clone(),
            calls[1][2].clone(),
            text.lines()
                .filter(|line| line.starts_with("[block"))
                .count(),
            text.contains(&other.chapter_id),
            question_attempts(&admin, &ours.scope).await,
            sent(&admin, &run_id).await[1][2][1].clone(),
            outputs_record(&admin, &run_id, 1).await[1].clone(),
            no_public_call(&store, &admin, &ours.scope, &run_id, &ids[1]).await,
        ),
        (
            tools(true),
            json!(["hybrid_search_manuscript", "settled", null, text, true]),
            finish(),
            1,
            false,
            0,
            output,
            json!([
                ["manuscript_search_result", ids[0], "exact_required"],
                ["manuscript_search_result", ids[1], "exact_required"],
            ]),
            true,
        )
    );
    assert_eq!(calls[1][0][0], "search_manuscript");
}

/// ADR 0091 section 3: an intake refusal of each field, and `hybrid_search_unavailable` when the
/// deployment has no embedding configuration at the call. Each Run continues and completes.
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

    let text = exact_list(&store, &chapter.scope, &["灯塔", "渔船"]).await;
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
