use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    NoContractFaults, ProjectScope, ResearchArtifactReadStore, complete_agent_run,
    complete_agent_run_with_tools,
};
use storyos_core::{
    Citation, DeploymentDestination, FINISH_DOES_NOT_COUNT_RESULT, RESEARCH_RECORDED_RESULT,
    RESEARCH_TOOL_OFFER, ResearchClaim, ResearchSynthesis, SubmitResearchRefusal, SynthesisRefusal,
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

    /// The status of the Run, the decision kind and `tool_choice` of each RunStep, each ToolCall
    /// as its function name, state, refusal, and Host result, and the citation kinds of each
    /// Research Artifact of the Run.
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
                                      CASE WHEN call.registration_id = 'host_tools'
                                           THEN call.result_text END)
                                      ORDER BY call.decision_position, call.call_position)
                                      FROM storyos.tool_calls AS call
                                     WHERE call.run_id = run.run_id),
                          'artifacts', (SELECT jsonb_agg(jsonb_path_query_array(
                                          artifact.content, '$.claims[*].citations[*].kind'))
                                          FROM storyos.research_artifacts AS artifact
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

/// The arguments of a synthesis with one claim that cites `citations`.
fn synthesis(summary: &str, citations: Value) -> Value {
    json!({"summary": summary, "claims": [{"text": "High water came at 10.42.", "citations": citations}]})
}

/// The facts of a completed Run whose first RunStep made the `research` call and whose next
/// `required` RunSteps made the Host `calls`, before the closing RunStep.
fn completed(research: &str, required: usize, calls: Value, artifacts: Value) -> Value {
    let mut steps = vec![json!(["tool_request", "required"]); required + 1];
    steps.push(json!(["advisory", "none"]));
    let mut all = vec![json!([research, "settled", null, null])];
    all.extend(calls.as_array().unwrap().iter().cloned());
    json!({"status": "completed", "steps": steps, "calls": all, "artifacts": artifacts})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_valid_call_records_one_artifact_with_the_lineage_of_its_agent_decision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("870").await;
    let arguments = synthesis("The tides of 1890.", json!(["$SOURCE"]));
    let run_id = project
        .settle("871", &script("once", "web_search", arguments), SEARCH_TEXT)
        .await;

    // The lineage names the RunStep, the Model Attempt, and the Agent Decision that hold the
    // call, and the research ToolCall whose result that RunStep read. The claim cites the
    // identity that the model read in the `tool_sources` tag of that result.
    let lineage = project
        .admin
        .query_one(
            "SELECT requirement.operation_requirement_id::text, attempt.model_attempt_id::text,
                    attempt.decision_id::text, search.tool_call_id::text,
                    snapshot.source_snapshot_id::text, closing.continuation_binding_id::text
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
        .is_some();

    assert_eq!(
        (
            project.facts(&run_id).await,
            artifact,
            project.decision(&run_id).await,
            read,
        ),
        (
            completed(
                "web_search",
                1,
                json!([
                    ["submit_research", "settled", null, RESEARCH_RECORDED_RESULT],
                    ["finish", "settled", null, ""]
                ]),
                json!([["snapshot"]]),
            ),
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
            true,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_call_gives_its_reason_and_one_run_records_one_artifact() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("874").await;
    let refused = |refusal: SubmitResearchRefusal| {
        json!([
            "submit_research",
            "refused",
            refusal.label(),
            refusal.result().text
        ])
    };
    let invalid = |refusal| refused(SubmitResearchRefusal::Synthesis(refusal));
    let recorded = json!(["submit_research", "settled", null, RESEARCH_RECORDED_RESULT]);
    let not_counted = json!(["finish", "settled", null, FINISH_DOES_NOT_COUNT_RESULT]);
    let finish = json!(["finish", "settled", null, ""]);
    let cited = synthesis("The tides of 1890.", json!(["$SOURCE"]));
    let uncaptured = synthesis(
        "The tides.",
        json!(["018f0000-0000-7001-8000-0000000875ff"]),
    );
    // Core proves each rule of the validation and of the typed intake. The model reads the
    // reason of a refused call, and its next `finish` call counts.
    let cases = [
        (
            "875",
            script("once", "web_search", uncaptured),
            SEARCH_TEXT,
            completed(
                "web_search",
                2,
                json!([
                    invalid(SynthesisRefusal::UncapturedCitation),
                    not_counted,
                    finish
                ]),
                Value::Null,
            ),
        ),
        // The Host executes `finish` after the other Host Tool calls of its response.
        (
            "876",
            script(
                "finish_first",
                "web_search",
                synthesis("The tides.", json!([])),
            ),
            SEARCH_TEXT,
            completed(
                "web_search",
                2,
                json!([not_counted, invalid(SynthesisRefusal::UncitedClaim), finish]),
                Value::Null,
            ),
        ),
        (
            "877",
            script("twice", "web_search", cited.clone()),
            SEARCH_TEXT,
            completed(
                "web_search",
                2,
                json!([
                    recorded,
                    refused(SubmitResearchRefusal::SecondInResponse),
                    not_counted,
                    finish
                ]),
                json!([["snapshot"]]),
            ),
        ),
        (
            "878",
            script("again", "web_search", cited),
            SEARCH_TEXT,
            completed(
                "web_search",
                3,
                json!([
                    recorded,
                    refused(SubmitResearchRefusal::ArtifactRecorded),
                    not_counted,
                    finish
                ]),
                json!([["snapshot"]]),
            ),
        ),
        (
            "879",
            script("once", "web_fetch", synthesis("No page.", json!(["$GAP"]))),
            FAILED_PAGE,
            completed("web_fetch", 1, json!([recorded, finish]), json!([["gap"]])),
        ),
    ];
    let mut observed = Vec::new();
    let mut want = Vec::new();
    for (prefix, message, text, facts) in cases {
        let run_id = project.settle(prefix, &message, text).await;
        observed.push(project.facts(&run_id).await);
        want.push(facts);
    }

    assert_eq!(observed, want);
}
