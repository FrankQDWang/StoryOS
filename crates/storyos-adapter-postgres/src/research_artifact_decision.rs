//! The public `decision` of a Run of the tool loop profile that recorded a Research Artifact
//! (ADR 0091 section 6): `research_synthesis` with that Artifact, in place of the final answer
//! text. The continuation binding stays the one of the final answer, which the next Run of the
//! Conversation continues from.

use storyos_application::{AgentRunDecisionInspect, CreateAgentRunError, ProjectScope};

use crate::create_agent_run::agent_run_database_error;

/// Replaces the final answer `decision` of the Run `run_id` with the research synthesis of its
/// Research Artifact, when the Run has one.
pub(crate) async fn derive(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
    decision: &mut AgentRunDecisionInspect,
) -> Result<(), CreateAgentRunError> {
    let AgentRunDecisionInspect::Advisory {
        continuation_binding_id,
        ..
    } = decision
    else {
        return Ok(());
    };
    let Some(row) = client
        .query_opt(
            "SELECT run.model_registration_revision::text, artifact.research_artifact_id::text,
                    artifact.decision_id::text
               FROM storyos.agent_runs AS run
               JOIN storyos.research_artifacts AS artifact
                 ON (artifact.owner_user_id, artifact.project_id, artifact.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?
    else {
        return Ok(());
    };
    if !storyos_core::model_registration(row.get(/*idx*/ 0))
        .is_some_and(storyos_core::ModelRegistration::declares_host_tools)
    {
        return Ok(());
    }
    *decision = AgentRunDecisionInspect::ResearchSynthesis {
        decision_id: row.get(/*idx*/ 2),
        selected: true,
        research_artifact_id: row.get(/*idx*/ 1),
        continuation_binding_id: continuation_binding_id.take(),
    };
    Ok(())
}
