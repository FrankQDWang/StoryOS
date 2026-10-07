use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use tokio_postgres::Client;

use super::LoadedGeneration;
use crate::command_sequence::unavailable;

/// The Proposal Generation, state, and AgentRun that one transition record moves to.
pub(super) struct ResultingGeneration<'a> {
    pub(super) event_id: &'a str,
    pub(super) generation_id: &'a str,
    pub(super) state: &'a str,
    pub(super) run_id: &'a str,
}

/// Inserts the queued successor of a terminal AgentRun and copies its Context Assembly.
pub(super) async fn insert_successor_run(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    loaded: &LoadedGeneration,
    run_id: &str,
) -> Result<(), ProjectCommandError> {
    let scope = &envelope.project_scope;
    let receipt_id = &envelope.ids.receipt_id;
    client
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id, predecessor_run_id,
                wakeup_pending)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10, 'queued', $11::text::uuid, $12::text::uuid, true)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &loaded.run_agent_id,
                &loaded.conversation_id,
                &loaded.memory_settings_revision,
                &loaded.grant_id,
                &loaded.binding_revision,
                &loaded.chapter_id,
                &loaded.author_message,
                &receipt_id,
                &loaded.source_run_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    copy_successor_context(client, scope, &loaded.source_run_id, run_id, receipt_id).await?;
    Ok(())
}

async fn copy_successor_context(
    client: &Client,
    scope: &ProjectScope,
    predecessor_run_id: &str,
    run_id: &str,
    receipt_id: &str,
) -> Result<(), ProjectCommandError> {
    let requirement_id = uuid::Uuid::now_v7().to_string();
    let snapshot_id = uuid::Uuid::now_v7().to_string();
    let manifest_id = uuid::Uuid::now_v7().to_string();
    let rebound = "jsonb_set(jsonb_set(jsonb_set(payload,
                      '{operation_requirement,run_id}', to_jsonb($5::text), true),
                      '{operation_requirement,operation_requirement_id}', to_jsonb($4::text), true),
                      '{operation_requirement,input_snapshot_id}', to_jsonb($6::text), true)";
    let requirement = client
        .execute(
            &format!(
                "INSERT INTO storyos.operation_requirements
                   (owner_user_id, project_id, operation_requirement_id, run_id,
                    input_snapshot_id, receipt_id, payload)
                 SELECT owner_user_id, project_id, $4::text::uuid, $5::text::uuid,
                        $6::text::uuid, $7::text::uuid, {rebound}
                   FROM storyos.operation_requirements
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND requirement_role = 'primary'
                    AND decision_position=(SELECT active_decision_position FROM storyos.agent_runs WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND run_id=$3::text::uuid)"
            ),
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &predecessor_run_id,
                &requirement_id,
                &run_id,
                &snapshot_id,
                &receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if requirement != 1 {
        return Err(unavailable("The predecessor Context Assembly is missing"));
    }
    let assembly = client
        .execute(
            &format!(
                "INSERT INTO storyos.context_assembly_manifests
                   (owner_user_id, project_id, context_assembly_manifest_id,
                    operation_requirement_id, run_id, sufficiency,
                    destination_context_manifest_id, outbound_disclosure_manifest_id,
                    payload, receipt_id)
                 SELECT owner_user_id, project_id, $8::text::uuid, $4::text::uuid,
                        $5::text::uuid, sufficiency, NULL, NULL, {rebound}, $7::text::uuid
                   FROM storyos.context_assembly_manifests
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND manifest_role = 'decision'
                    AND decision_position=(SELECT active_decision_position FROM storyos.agent_runs WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND run_id=$3::text::uuid)"
            ),
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &predecessor_run_id,
                &requirement_id,
                &run_id,
                &snapshot_id,
                &receipt_id,
                &manifest_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if assembly != 1 {
        return Err(unavailable("The predecessor Context Assembly is missing"));
    }
    Ok(())
}

/// Inserts the Proposal Generation transition record of one applied decision.
pub(super) async fn insert_transition(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    author_action_sequence: u64,
    proposal_id: &str,
    loaded: &LoadedGeneration,
    resulting: ResultingGeneration<'_>,
) -> Result<(), ProjectCommandError> {
    let scope = &envelope.project_scope;
    client
        .execute(
            "INSERT INTO storyos.proposal_generation_transitions
               (owner_user_id, project_id, transition_id, proposal_id, proposal_revision_id,
                prior_generation_id, resulting_generation_id, prior_generation_state,
                resulting_generation_state, prior_run_id, resulting_run_id,
                preserved_validation, preserved_closure, preserved_operation_resolution,
                receipt_id, author_action_sequence)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8, $9,
                     $10::text::uuid, $11::text::uuid, $12, $13, $14, $15::text::uuid,
                     $16::text::numeric)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &resulting.event_id,
                &proposal_id,
                &loaded.revision_id,
                &loaded.generation_id,
                &resulting.generation_id,
                &loaded.generation_state,
                &resulting.state,
                &loaded.source_run_id,
                &resulting.run_id,
                &loaded.validation,
                &loaded.closure,
                &loaded.operation_resolution,
                &envelope.ids.receipt_id,
                &author_action_sequence.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}
