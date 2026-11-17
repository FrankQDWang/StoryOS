//! The Embedding Attempt (ADR 0095 section 3): the row that the claim commits before the exchange,
//! its settlement, one store test for each typed refusal, the route of the binding, and the
//! Project Scope. Test prefix lead `ab` (`ab0`, and `ab7` and later;
//! `project_embedding_binding_tests` uses `ab1` to `ab6`).

use std::sync::Mutex;

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_application::{
    CredentialReference, EmbeddingDispatchStore as _, EmbeddingObservation,
    EmbeddingProviderAdapter, EmbeddingResult, EmbeddingRoute, NoEmbeddingDestination,
    PreDispatchRefusal, PreparedEmbeddingRequest, ProjectScope, dispatch_embedding,
};
use storyos_core::{
    AGENT_PLAN_EMBEDDING_REGISTRATION, EmbeddingInput, EmbeddingRefusal, EmbeddingRegistration,
    EmbeddingSettlement, HOST_FAKE_EMBEDDING_REGISTRATION,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::project_embedding_binding::tests::{FAKE, embedding_project};
use crate::project_research_binding::tests::park_queued_runs;

/// The fake embedding adapter. Its exchange first reads the Embedding Attempts of the Project
/// through `admin`; `truncate` cuts the vector to 1,023 values.
struct Held<'a> {
    admin: &'a Client,
    project_id: String,
    truncate: bool,
    seen: Mutex<Vec<Value>>,
}

impl EmbeddingProviderAdapter for Held<'_> {
    type Prepared = <FakeEmbeddings as EmbeddingProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, PreDispatchRefusal> {
        FakeEmbeddings.prepare(registration, route, input).await
    }

    async fn exchange(&self, prepared: Self::Prepared) -> EmbeddingObservation {
        let rows = attempts(self.admin, &self.project_id).await;
        self.seen.lock().unwrap().push(rows);
        match FakeEmbeddings.exchange(prepared).await {
            EmbeddingObservation::Vector { mut values, usage } if self.truncate => {
                values.pop();
                EmbeddingObservation::Vector { values, usage }
            }
            observation => observation,
        }
    }
}

/// Each Embedding Attempt of the Project in the order of its creation.
async fn attempts(admin: &Client, project_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                      'cause', cause, 'settlement', settlement, 'attempt', attempt)
                      ORDER BY embedding_attempt_id), '[]'::jsonb)::text
               FROM storyos.embedding_attempts WHERE project_id = $1::text::uuid",
            &[&project_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The Project of `embedding_project`, with the head revision of its Chapter.
async fn project(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    embedding: Option<&'static EmbeddingRegistration>,
) -> (ProjectScope, String, String, String) {
    let (scope, chapter_id, run_id) = embedding_project(store, admin, prefixes, embedding).await;
    let revision_id: String = admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    (scope, chapter_id, revision_id, run_id)
}

fn passage(chapter_id: &str, revision_id: &str, text: &str) -> EmbeddingInput {
    EmbeddingInput::Passage {
        chapter_id: chapter_id.to_owned(),
        revision_id: revision_id.to_owned(),
        passage_digest: "sha256:passage".to_owned(),
        text: text.to_owned(),
    }
}

fn question(tool_call_id: &str) -> EmbeddingInput {
    EmbeddingInput::Question {
        tool_call_id: tool_call_id.to_owned(),
        text: "Where is the lantern?".to_owned(),
    }
}

/// The number of Embedding Attempts that a read in the Scope of `scope` gets.
async fn scoped_count(store: &PostgresProjectReader, scope: &ProjectScope) -> i64 {
    store
        .in_read_transaction(scope, async |client| {
            Ok(client
                .query_one("SELECT count(*) FROM storyos.embedding_attempts", &[])
                .await
                .unwrap()
                .get(/*idx*/ 0))
        })
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_claim_commits_the_attempt_before_the_exchange_and_the_record_settles_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, revision_id, run_id) =
        project(&store, &admin, ["ab7", "ab8"], FAKE).await;
    let (other, ..) = project(&store, &admin, ["ab9", "aba"], FAKE).await;
    park_queued_runs(&admin, &other).await;
    // The finish rule of the fake tool loop gives the Run one ToolCall.
    complete(&store, &admin, &scope, &run_id).await;
    let tool_call_id: String = admin
        .query_one(
            "SELECT tool_call_id::text FROM storyos.tool_calls WHERE run_id = $1::text::uuid
              ORDER BY tool_call_id LIMIT 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let held = |truncate| Held {
        admin: &admin,
        project_id: scope.project_id.as_ref().to_owned(),
        truncate,
        seen: Mutex::new(Vec::new()),
    };
    let first = held(false);
    let dispatch = async |adapter: &Held<'_>, input: EmbeddingInput| {
        dispatch_embedding(&store, adapter, FAKE, &scope, &input)
            .await
            .unwrap()
    };

    let vector = dispatch(
        &first,
        passage(&chapter_id, &revision_id, "The tide turned."),
    )
    .await;
    let during = first.seen.lock().unwrap()[0].clone();
    let after = attempts(&admin, scope.project_id.as_ref()).await;
    let results = [
        dispatch(&held(false), question(&tool_call_id)).await,
        dispatch(&held(true), question(&tool_call_id)).await,
        dispatch(
            &held(false),
            passage(&chapter_id, &revision_id, "SCRIPT:embedding-rejected"),
        )
        .await,
        dispatch(
            &held(false),
            passage(&chapter_id, &revision_id, "SCRIPT:embedding-not-submitted"),
        )
        .await,
        dispatch(
            &held(false),
            passage(&chapter_id, &revision_id, "SCRIPT:embedding-unknown"),
        )
        .await,
    ];

    let EmbeddingResult::Vector(values) = vector else {
        panic!("not a vector: {vector:?}");
    };
    assert_eq!(values.len(), 1_024);
    let attempt = &during[0]["attempt"];
    let admission = &attempt["admission_decision"];
    assert_eq!(
        (
            during[0]["settlement"].clone(),
            attempt["destination_attempt"]["settlement"].clone()
        ),
        (Value::Null, Value::Null)
    );
    assert_eq!(
        (
            admission["decision"].clone(),
            admission["grant_scope"].clone(),
            admission["embedding_registration"].clone(),
            admission["request_bounds"].clone(),
            admission["evidence_revision"].clone(),
        ),
        (
            json!("admitted"),
            json!("manuscript_embedding_v1"),
            json!(HOST_FAKE_EMBEDDING_REGISTRATION.revision),
            json!("embedding_request_v1"),
            json!("1"),
        )
    );
    assert_eq!(
        attempt["operation_input_snapshot"],
        json!({
            "cause": "passage_index", "purpose": "manuscript_embedding",
            "source_class": "chapter_passage",
            "texts": [{"chapter_id": chapter_id, "revision_id": revision_id,
                       "passage_digest": "sha256:passage"}],
            "working_target": "not_applicable", "workspace_context": "not_applicable",
        })
    );
    assert_eq!(
        (
            attempt["wire_payload_projection"]["mapping_revision"].clone(),
            attempt["wire_payload_projection"]
                .to_string()
                .contains("tide"),
            attempt["outbound_disclosure_event"]["state"].clone(),
            attempt["outbound_disclosure_event"]["source_class"].clone(),
        ),
        (
            json!("storyos.host-fake.embedding.v1"),
            false,
            json!("outcome_unknown"),
            json!("chapter_passage"),
        )
    );
    let settled = &after[0]["attempt"];
    assert_eq!(
        (
            after[0]["settlement"].clone(),
            settled["destination_attempt"]["settlement"].clone(),
            settled["usage"].clone(),
            settled["disclosure_confirmation"]["state"].clone(),
            settled["disclosure_confirmation"]["event_id"].clone(),
        ),
        (
            json!("vector"),
            json!("vector"),
            json!({"kind": "reported", "input_tokens": 16}),
            json!("confirmed"),
            attempt["outbound_disclosure_event"]["event_id"].clone(),
        )
    );
    assert!(matches!(&results[0], EmbeddingResult::Vector(values) if values.len() == 1_024));
    assert_eq!(
        results[1..],
        [
            EmbeddingResult::Settled(EmbeddingSettlement::InvalidResult),
            EmbeddingResult::Settled(EmbeddingSettlement::Rejected),
            EmbeddingResult::Settled(EmbeddingSettlement::NotSubmitted),
            EmbeddingResult::Settled(EmbeddingSettlement::OutcomeUnknown),
        ]
    );
    let rows = attempts(&admin, scope.project_id.as_ref()).await;
    let summary: Vec<Value> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            json!([
                row["cause"],
                row["settlement"],
                row["attempt"]["usage"]["kind"],
                row["attempt"]["rejection"],
                row["attempt"]["disclosure_confirmation"]["state"],
            ])
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            json!(["passage_index", "vector", "reported", null, "confirmed"]),
            json!(["search_question", "vector", "reported", null, "confirmed"]),
            json!([
                "search_question",
                "invalid_result",
                "reported",
                null,
                "confirmed"
            ]),
            json!([
                "passage_index",
                "rejected",
                "unknown",
                "fake_rejection",
                "confirmed"
            ]),
            json!([
                "passage_index",
                "not_submitted",
                "unknown",
                null,
                "not_disclosed"
            ]),
            json!([
                "passage_index",
                "outcome_unknown",
                "unknown",
                null,
                "outcome_unknown"
            ]),
        ]
    );
    assert_eq!(
        rows[1]["attempt"]["operation_input_snapshot"]["texts"],
        json!([{"tool_call_id": tool_call_id}])
    );
    // A read in the Scope of another Project does not get the rows.
    assert_eq!(
        [
            scoped_count(&store, &scope).await,
            scoped_count(&store, &other).await
        ],
        [6, 0]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refusal_writes_no_attempt_and_gives_its_typed_reason() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, revision_id, run_id) =
        project(&store, &admin, ["abb", "abc"], FAKE).await;
    let (foreign, foreign_chapter, foreign_revision, _) =
        project(&store, &admin, ["abd", "abe"], FAKE).await;
    park_queued_runs(&admin, &foreign).await;
    complete(&store, &admin, &scope, &run_id).await;
    let own_call: String = admin
        .query_one(
            "SELECT tool_call_id::text FROM storyos.tool_calls WHERE run_id = $1::text::uuid
              LIMIT 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let text = |text: &str| passage(&chapter_id, &revision_id, text);
    let refusal = async |registration, scope: &ProjectScope, input: EmbeddingInput| {
        dispatch_embedding(&store, &FakeEmbeddings, registration, scope, &input)
            .await
            .unwrap()
    };

    let mut results = vec![
        refusal(None, &scope, text("The tide turned.")).await,
        // The Project has no use binding for the destination of the Agent Plan Registration.
        refusal(
            Some(&AGENT_PLAN_EMBEDDING_REGISTRATION),
            &scope,
            text("The tide turned."),
        )
        .await,
        // A Chapter, a Revision, and a ToolCall of another Project.
        refusal(
            FAKE,
            &scope,
            passage(&foreign_chapter, &foreign_revision, "x"),
        )
        .await,
        refusal(FAKE, &scope, passage(&chapter_id, &foreign_revision, "x")).await,
        refusal(FAKE, &foreign, question(&own_call)).await,
        refusal(FAKE, &scope, text("")).await,
        refusal(FAKE, &scope, text(&"潮".repeat(8_193))).await,
    ];
    results.push(
        dispatch_embedding(
            &store,
            &NoEmbeddingDestination,
            FAKE,
            &scope,
            &text("The tide turned."),
        )
        .await
        .unwrap(),
    );
    // The binding of the Project names the grant of the writing assistance: its scope does not
    // name the Purpose of the embedding request.
    admin
        .execute(
            "UPDATE storyos.project_external_use_binding_revisions AS binding
                SET grant_id = (SELECT grant_id FROM storyos.project_destination_grants
                                 WHERE project_id = binding.project_id
                                   AND grant_kind = 'host_fake_use' LIMIT 1)
              WHERE binding.project_id = $1::text::uuid
                AND binding.embedding_registration_revision IS NOT NULL",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    results.push(refusal(FAKE, &scope, text("The tide turned.")).await);

    assert_eq!(
        results,
        [
            EmbeddingRefusal::NotConfigured,
            EmbeddingRefusal::BindingAbsent,
            EmbeddingRefusal::DisclosureOutsideGrant,
            EmbeddingRefusal::DisclosureOutsideGrant,
            EmbeddingRefusal::DisclosureOutsideGrant,
            EmbeddingRefusal::InputOutOfBounds,
            EmbeddingRefusal::InputOutOfBounds,
            EmbeddingRefusal::CredentialUnavailable,
            EmbeddingRefusal::DisclosureOutsideGrant,
        ]
        .map(EmbeddingResult::Refused)
    );
    assert_eq!(
        [
            attempts(&admin, scope.project_id.as_ref()).await,
            attempts(&admin, foreign.project_id.as_ref()).await,
        ],
        [json!([]), json!([])]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_route_is_the_endpoint_of_the_evidence_and_the_reference_of_the_current_binding() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (agent_plan, ..) = project(
        &store,
        &admin,
        ["ab0", "abf"],
        Some(&AGENT_PLAN_EMBEDDING_REGISTRATION),
    )
    .await;
    park_queued_runs(&admin, &agent_plan).await;

    assert_eq!(
        store.embedding_route(&agent_plan).await.unwrap(),
        Some(EmbeddingRoute {
            registration_revision: AGENT_PLAN_EMBEDDING_REGISTRATION.revision.to_owned(),
            endpoint: Some(storyos_core::AGENT_PLAN_ENDPOINT.to_owned()),
            credential_reference: Some(CredentialReference(
                storyos_core::AGENT_PLAN_CREDENTIAL_REFERENCE.to_owned()
            )),
        })
    );
}
