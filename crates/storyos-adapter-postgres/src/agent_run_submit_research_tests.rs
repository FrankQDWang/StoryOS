use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    NoContractFaults, ProjectScope, ResearchArtifactReadStore, complete_agent_run,
    complete_agent_run_with_tools,
};
use storyos_core::{
    CallValidationRefusal, Citation, DeploymentDestination, FINISH_DOES_NOT_COUNT_RESULT,
    IntakeRefusal, RESEARCH_RECORDED_RESULT, RESEARCH_TOOL_OFFER, ResearchClaim, ResearchSynthesis,
    SubmitResearchRefusal, SynthesisRefusal,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};
use crate::agent_run_retrieval::tests::destination;
use crate::agent_run_tool_dispatch::tests::{Hooked, exa_tools, mcp_replies, point_mcp, reply};
use crate::research_source::tests::SEARCH_TEXT;

const FAILED_PAGE: &str = "Error fetching https://example.org/c: CRAWL_NOT_FOUND";

/// One Project of the tool loop profile with the research offer. Its first Run completed.
struct ResearchProject {
    store: PostgresProjectReader,
    admin: Client,
    scope: ProjectScope,
    chapter_id: String,
}

async fn research_project(prefix: &str) -> ResearchProject {
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued_run_offering(
        &store,
        prefix,
        DeploymentDestination::HostFakeToolLoop,
        Some(&RESEARCH_TOOL_OFFER),
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), first).await;
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    ResearchProject {
        store,
        admin,
        scope,
        chapter_id,
    }
}

impl ResearchProject {
    /// Settles one Run with `message` through the fake model and the real MCP client. The fake
    /// MCP server answers the one research call of the Run with `text`.
    async fn settle(&self, prefix: &str, message: &str, text: &str) -> String {
        let run_id = admit_run_in(
            &self.store,
            &self.scope,
            &self.chapter_id,
            prefix,
            DeploymentDestination::HostFakeToolLoop,
            message,
            ConversationSelection::New,
            Some(&RESEARCH_TOOL_OFFER),
        )
        .await;
        let result = reply(
            /*id*/ 0,
            json!({"content": [{"type": "text", "text": text}], "isError": false}),
        );
        let (mcp_base, _mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(result))).await;
        point_mcp(&self.admin, &self.scope, &mcp_base).await;
        let claim = claim_run(&self.admin, self.scope.clone(), run_id.clone()).await;
        let tools = Hooked {
            admin: &self.admin,
            after_prepare: String::new(),
            after_exchange: String::new(),
        };
        complete_agent_run_with_tools(
            &self.store,
            &FakeDestination,
            &tools,
            &NoContractFaults,
            &RuntimeClock,
            &claim,
        )
        .await
        .unwrap();
        run_id
    }

    /// The status of the Run, the decision kind of each RunStep, each ToolCall as its function
    /// name, state, refusal, and result, and the number of its Research Artifacts.
    async fn facts(&self, run_id: &str) -> Value {
        let row = self
            .admin
            .query_one(
                "SELECT jsonb_build_object(
                          'status', run.status,
                          'steps', (SELECT jsonb_agg(jsonb_build_array(
                                      attempt.payload #>> '{decision,kind}',
                                      (attempt.payload #>> '{wire,serialized_payload}')::jsonb
                                        ->> 'tool_choice')
                                      ORDER BY attempt.model_attempt_id)
                                      FROM storyos.model_attempts AS attempt
                                     WHERE attempt.run_id = run.run_id),
                          'calls', (SELECT jsonb_agg(jsonb_build_array(
                                      call.function_name, call.state, call.refusal,
                                      CASE WHEN call.function_name = 'web_search' THEN NULL
                                           ELSE call.result_text END)
                                      ORDER BY call.decision_position, call.call_position)
                                      FROM storyos.tool_calls AS call
                                     WHERE call.run_id = run.run_id),
                          'artifacts', (SELECT count(*) FROM storyos.research_artifacts AS artifact
                                         WHERE artifact.run_id = run.run_id))::text
                   FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap();
        serde_json::from_str(row.get::<_, &str>(0)).unwrap()
    }

    async fn decision(&self, run_id: &str) -> AgentRunDecisionInspect {
        self.store
            .read_agent_run(&self.scope, run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap()
            .decision
    }
}

fn script(variant: &str, first_tool: &str, arguments: Value) -> String {
    format!("SCRIPT:submit_research {variant} {first_tool} {arguments}")
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_valid_call_records_one_artifact_with_the_lineage_of_its_agent_decision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("870").await;
    let arguments = json!({"summary": "The tides of 1890.",
        "claims": [{"text": "High water came at 10.42.", "citations": ["$SOURCE"]}]});
    let run_id = project
        .settle("871", &script("once", "web_search", arguments), SEARCH_TEXT)
        .await;

    // The lineage names the RunStep, the Model Attempt, and the Agent Decision that hold the
    // call, and the research ToolCall whose result that RunStep read.
    let lineage = project
        .admin
        .query_one(
            "SELECT requirement.operation_requirement_id::text, attempt.model_attempt_id::text,
                    attempt.decision_id::text, search.tool_call_id::text,
                    snapshot.source_snapshot_id::text, closing.continuation_binding_id::text,
                    (attempt.payload #>> '{wire,serialized_payload}')::jsonb
                      #>> '{tool_outputs,0,output}'
               FROM storyos.model_attempts AS attempt
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.run_id, requirement.decision_position,
                     requirement.requirement_role) =
                    (attempt.run_id, attempt.decision_position, 'primary')
               JOIN storyos.tool_calls AS search
                 ON search.run_id = attempt.run_id AND search.function_name = 'web_search'
               JOIN storyos.source_snapshots AS snapshot
                 ON snapshot.tool_call_id = search.tool_call_id AND snapshot.entry_position = 0
               JOIN storyos.model_attempts AS closing
                 ON closing.run_id = attempt.run_id AND closing.decision_position = 2
              WHERE attempt.run_id = $1::text::uuid AND attempt.decision_position = 1",
            &[&run_id],
        )
        .await
        .unwrap();
    let text = |index| lineage.get::<_, String>(index);
    let synthesis = ResearchSynthesis {
        summary: "The tides of 1890.".to_owned(),
        claims: vec![ResearchClaim {
            text: "High water came at 10.42.".to_owned(),
            citations: vec![Citation::Snapshot {
                source_snapshot_id: text(4),
            }],
        }],
    };
    let artifact: Value = serde_json::from_str(
        project
            .admin
            .query_one(
                "SELECT (to_jsonb(artifact) - 'owner_user_id' - 'project_id'
                          - 'research_artifact_id' - 'created_at')::text
                   FROM storyos.research_artifacts AS artifact WHERE run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap()
            .get::<_, &str>(0),
    )
    .unwrap();
    let AgentRunDecisionInspect::ResearchSynthesis {
        research_artifact_id,
        ..
    } = project.decision(&run_id).await
    else {
        panic!("the Run that recorded an Artifact shows a research synthesis");
    };
    let read = project
        .store
        .get_research_artifact(&project.scope, &research_artifact_id)
        .await
        .unwrap()
        .expect("the panel reads the Artifact");

    assert_eq!(
        (
            project.facts(&run_id).await,
            artifact,
            project.decision(&run_id).await,
            read.lineage.decision_id,
            text(6).contains(r#"<tool_sources source="call_web_search"><source id=""#),
        ),
        (
            json!({
                "status": "completed",
                "steps": [["tool_request", "required"], ["tool_request", "required"],
                          ["advisory", "none"]],
                "calls": [["web_search", "settled", null, null],
                          ["submit_research", "settled", null, RESEARCH_RECORDED_RESULT],
                          ["finish", "settled", null, ""]],
                "artifacts": 1,
            }),
            json!({
                "run_id": run_id, "decision_position": 1,
                "operation_requirement_id": text(0), "model_attempt_id": text(1),
                "decision_id": text(2), "tool_call_ids": [text(3)],
                "content": synthesis.content(), "content_digest": synthesis.content_digest(),
            }),
            AgentRunDecisionInspect::ResearchSynthesis {
                decision_id: text(2),
                selected: true,
                research_artifact_id,
                continuation_binding_id: Some(text(5)),
            },
            text(2),
            true,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_whose_claims_cite_only_gaps_records_a_partial_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("872").await;
    let arguments = json!({"summary": "No tide table is available.",
        "claims": [{"text": "The page could not be read.", "citations": ["$GAP"]}]});
    let run_id = project
        .settle("873", &script("once", "web_fetch", arguments), FAILED_PAGE)
        .await;
    let gap: String = project
        .admin
        .query_one(
            "SELECT source_gap_id::text FROM storyos.source_gaps
              WHERE run_id = $1::text::uuid AND gap_position = 0",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let content: Value = serde_json::from_str(
        project
            .admin
            .query_one(
                "SELECT content::text FROM storyos.research_artifacts WHERE run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap()
            .get::<_, &str>(0),
    )
    .unwrap();

    assert_eq!(
        (project.facts(&run_id).await["calls"].clone(), content),
        (
            json!([
                ["web_fetch", "settled", null, FAILED_PAGE],
                ["submit_research", "settled", null, RESEARCH_RECORDED_RESULT],
                ["finish", "settled", null, ""]
            ]),
            ResearchSynthesis {
                summary: "No tide table is available.".to_owned(),
                claims: vec![ResearchClaim {
                    text: "The page could not be read.".to_owned(),
                    citations: vec![Citation::Gap { source_gap_id: gap }],
                }],
            }
            .content(),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_call_gives_its_reason_as_the_result_and_writes_no_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("874").await;
    let claim =
        |citations: Value| json!([{"text": "High water came at 10.42.", "citations": citations}]);
    let synthesis = |refusal| SubmitResearchRefusal::Synthesis(refusal).result().text;
    let cases = [
        (
            "875",
            json!({"summary": "The tides.", "claims": []}),
            ("no_claim", synthesis(SynthesisRefusal::NoClaim)),
        ),
        (
            "876",
            json!({"summary": " ", "claims": claim(json!(["$SOURCE"]))}),
            ("empty_text", synthesis(SynthesisRefusal::EmptyText)),
        ),
        (
            "877",
            json!({"summary": "The\u{0}tides.", "claims": claim(json!(["$SOURCE"]))}),
            (
                "unstorable_text",
                CallValidationRefusal::Intake(IntakeRefusal::UnstorableText).result_text(),
            ),
        ),
        (
            "878",
            json!({"summary": "The tides.", "claims": claim(json!([]))}),
            ("uncited_claim", synthesis(SynthesisRefusal::UncitedClaim)),
        ),
        (
            "879",
            json!({"summary": "The tides.",
                   "claims": claim(json!(["018f0000-0000-7001-8000-0000000879ff"]))}),
            (
                "uncaptured_citation",
                synthesis(SynthesisRefusal::UncapturedCitation),
            ),
        ),
    ];
    let mut observed = Vec::new();
    let mut want = Vec::new();
    for (prefix, arguments, (label, result)) in cases {
        let run_id = project
            .settle(
                prefix,
                &script("once", "web_search", arguments),
                SEARCH_TEXT,
            )
            .await;
        observed.push((
            project.facts(&run_id).await,
            matches!(
                project.decision(&run_id).await,
                AgentRunDecisionInspect::Advisory { .. }
            ),
        ));
        // The model reads the reason, and its next `finish` call counts.
        want.push((
            json!({
                "status": "completed",
                "steps": [["tool_request", "required"], ["tool_request", "required"],
                          ["tool_request", "required"], ["advisory", "none"]],
                "calls": [["web_search", "settled", null, null],
                          ["submit_research", "refused", label, result],
                          ["finish", "settled", null, FINISH_DOES_NOT_COUNT_RESULT],
                          ["finish", "settled", null, ""]],
                "artifacts": 0,
            }),
            true,
        ));
    }

    assert_eq!(observed, want);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn one_response_and_one_run_accept_one_call_that_records_an_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("87a").await;
    let arguments = json!({"summary": "The tides of 1890.",
        "claims": [{"text": "High water came at 10.42.", "citations": ["$SOURCE"]}]});
    let refused = |refusal: SubmitResearchRefusal| {
        json!([
            "submit_research",
            "refused",
            refusal.label(),
            refusal.result().text
        ])
    };
    let recorded = json!(["submit_research", "settled", null, RESEARCH_RECORDED_RESULT]);
    let mut observed = Vec::new();
    for (prefix, variant) in [("87b", "twice"), ("87c", "again")] {
        let run_id = project
            .settle(
                prefix,
                &script(variant, "web_search", arguments.clone()),
                SEARCH_TEXT,
            )
            .await;
        let facts = project.facts(&run_id).await;
        observed.push((facts["calls"].clone(), facts["artifacts"].clone()));
    }

    assert_eq!(
        observed,
        vec![
            (
                json!([
                    ["web_search", "settled", null, null],
                    recorded,
                    refused(SubmitResearchRefusal::SecondInResponse),
                    ["finish", "settled", null, FINISH_DOES_NOT_COUNT_RESULT],
                    ["finish", "settled", null, ""]
                ]),
                json!(1),
            ),
            (
                json!([
                    ["web_search", "settled", null, null],
                    recorded,
                    refused(SubmitResearchRefusal::ArtifactRecorded),
                    ["finish", "settled", null, FINISH_DOES_NOT_COUNT_RESULT],
                    ["finish", "settled", null, ""]
                ]),
                json!(1),
            ),
        ]
    );
}
