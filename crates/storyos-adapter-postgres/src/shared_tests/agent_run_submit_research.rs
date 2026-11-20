//! The shared test support from `agent_run_submit_research_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_retrieval::destination;
use crate::shared_tests::agent_run_tool_dispatch::{
    Hooked, exa_tools, mcp_replies, point_mcp, reply,
};
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run, queued_run_offering};
use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, NoContractFaults,
    ProjectScope, complete_agent_run, complete_agent_run_with_tools,
};
use storyos_core::{DeploymentDestination, RESEARCH_TOOL_OFFER};
use tokio_postgres::Client;

/// One Project of the tool loop profile with the research offer. Its `first` Run completed.
pub(crate) struct ResearchProject {
    pub store: PostgresProjectReader,
    pub admin: Client,
    pub scope: ProjectScope,
    pub(crate) chapter_id: String,
    pub(crate) first: String,
}

pub(crate) async fn research_project(prefix: &str) -> ResearchProject {
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued_run_offering(
        &store,
        prefix,
        DeploymentDestination::HostFakeToolLoop,
        Some(&RESEARCH_TOOL_OFFER),
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), first.clone()).await;
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
        first,
    }
}

impl ResearchProject {
    /// Settles one Run with `message` through the fake model and the real MCP client. The fake
    /// MCP server answers the one research call of the Run with `text`.
    pub(crate) async fn settle(&self, prefix: &str, message: &str, text: &str) -> String {
        let run_id = Admission {
            message,
            research: Some(&RESEARCH_TOOL_OFFER),
            ..Default::default()
        }
        .admit(&self.store, &self.scope, &self.chapter_id, prefix)
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
    pub(crate) async fn facts(&self, run_id: &str) -> Value {
        json_of(
            &self.admin,
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
    }

    pub(crate) async fn decision(&self, run_id: &str) -> AgentRunDecisionInspect {
        self.store
            .read_agent_run(&self.scope, run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap()
            .decision
    }
}
