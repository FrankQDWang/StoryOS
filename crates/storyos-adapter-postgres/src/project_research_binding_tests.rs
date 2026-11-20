use serde_json::{Value, json};
use storyos_application::{
    NoContractFaults, ProjectScope, complete_agent_run, complete_agent_run_with_tools,
};
use storyos_core::{DeploymentDestination, RESEARCH_TOOL_OFFER, ResearchToolOffer};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_retrieval::{adapter, destination, research_run};
use crate::shared_tests::agent_run_tool_call::{
    quarantine_statement, reactivate_research_revision, text_and_call_stream,
};
use crate::shared_tests::agent_run_tool_dispatch::{
    Hooked, RESULT, SEARCH, exa_tools, mcp_replies, reply, tool_run,
};
use crate::shared_tests::agent_run_tool_exposure::{advisory_stream, finish_stream, host_exposure};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run, queued_run_offering};
use crate::shared_tests::project_research_binding::park_queued_runs;

/// The same research destination with another Credential Reference.
static CHANGED_OFFER: ResearchToolOffer = ResearchToolOffer {
    credential_reference: "settings:STORYOS_EXA_API_KEY_ROTATED",
    ..RESEARCH_TOOL_OFFER
};

/// The command kinds of the Receipts of the Project, in order, and the number of its research
/// compatibility Decisions.
async fn project_facts(admin: &Client, scope: &ProjectScope) -> (Vec<String>, i64) {
    let row = admin
        .query_one(
            "SELECT ARRAY(SELECT command_kind::text FROM storyos.domain_receipts
                           WHERE project_id = $1::text::uuid ORDER BY created_at, receipt_id),
                    (SELECT count(*) FROM storyos.external_contract_compatibility_decisions
                      WHERE project_id = $1::text::uuid
                        AND decision_kind = 'tool_registration_compatible')",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// The function names of the Step Snapshot of the first decision Attempt of the Run.
async fn exposed_names(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT COALESCE(jsonb_agg(tool->'name'), '[]'::jsonb)::text
               FROM storyos.model_attempts AS attempt
               LEFT JOIN LATERAL jsonb_array_elements(
                 attempt.payload #> '{step_snapshot,tool_exposure,tools}') AS tool ON true
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'decision'
                AND attempt.decision_position = 0",
        &[&run_id],
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_first_run_of_a_project_binds_research_and_its_tool_call_dispatches() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = reply(
        0,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    );
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(call))).await;
    let (scope, run_id) = tool_run(&store, &admin, "6e1", &mcp_base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let tools = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let state: String = admin
        .query_one(
            "SELECT state FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND call_id = 'call_01'",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            project_facts(&admin, &scope).await,
            exposed_names(&admin, &run_id).await,
            state,
            mcp.await.unwrap().len(),
        ),
        (
            (
                [
                    "createProject",
                    "updateProjectAssistance",
                    "createVolume",
                    "createChapter",
                    "createAgentRun",
                ]
                .map(str::to_owned)
                .to_vec(),
                1,
            ),
            json!([
                "web_search",
                "web_fetch",
                "propose_edit",
                "submit_research",
                "read_tool_result",
                "read_chapter",
                "search_manuscript",
                "write_memory",
                "read_memory_entry",
                "search_memory",
                "finish"
            ]),
            "settled".to_owned(),
            4,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_run_writes_research_records_only_when_the_offer_changed() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let plan = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, chapter_id, _first) =
        queued_run_offering(&store, "6e2", plan.clone(), Some(&RESEARCH_TOOL_OFFER)).await;
    let after_first = project_facts(&admin, &scope).await.1;
    let mut counts = vec![after_first];
    for (prefix, offer) in [
        ("6e3", &RESEARCH_TOOL_OFFER),
        ("6e4", &CHANGED_OFFER),
        ("6e5", &CHANGED_OFFER),
    ] {
        Admission {
            destination: plan.clone(),
            research: Some(offer),
            ..Default::default()
        }
        .admit(&store, &scope, &chapter_id, prefix)
        .await;
        counts.push(project_facts(&admin, &scope).await.1);
    }
    park_queued_runs(&admin, &scope).await;

    assert_eq!(counts, vec![1, 1, 2, 2]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_quarantined_offer_binds_nothing_and_a_research_call_is_a_refused_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // A first Project writes the Registration rows that the quarantine changes.
    let (first, _) = research_run(&store, &admin, "6e6", "http://127.0.0.1:9/api/plan/v3").await;
    park_queued_runs(&admin, &first).await;
    admin.batch_execute(&quarantine_statement()).await.unwrap();
    let (base, served) = destination(vec![
        text_and_call_stream("resp_6e7", "call_01", "web_search", SEARCH, "completed"),
        finish_stream("resp_6e7b"),
        advisory_stream("resp_6e7c"),
    ])
    .await;
    let (scope, run_id) = research_run(&store, &admin, "6e7", &base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let completed =
        complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim).await;
    reactivate_research_revision(&admin).await;
    completed.unwrap();
    let settlement: Value = json_of(
        &admin,
        "SELECT jsonb_build_object('settlement', run.settlement,
                      'batch', attempt.payload->'tool_call_batch',
                      'snapshot', attempt.payload #> '{step_snapshot,tool_exposure}',
                      'refusal', (SELECT refusal FROM storyos.tool_calls
                                   WHERE run_id = run.run_id AND call_id = 'call_01'))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
                AND attempt.decision_position = 0
              WHERE run.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await;

    assert_eq!(
        (
            project_facts(&admin, &scope).await.1,
            settlement,
            served.await.unwrap().len()
        ),
        (
            0,
            json!({
                "settlement": null,
                "batch": {"kind": "valid", "call_ids": ["call_01"]},
                // Only the Host Tools stay exposed.
                "snapshot": host_exposure().snapshot(),
                "refusal": "unexposed_name",
            }),
            3,
        )
    );
}
