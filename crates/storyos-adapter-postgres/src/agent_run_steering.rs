use crate::agent_run_work::complete_database_error;
use crate::create_agent_run::context::{PassageContextInput, persist_current_passage_assembly};
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};

pub(crate) async fn advance(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    lease_seconds: i64,
) -> Result<bool, CompleteAgentRunError> {
    let Some(row) = client.query_opt(
        "SELECT run.author_message, run.chapter_id::text, input.receipt_id::text,
                input.payload->>'input_position'
           FROM storyos.agent_runs AS run
           JOIN storyos.project_activity_event_payloads AS input
             ON (input.owner_user_id,input.project_id)=(run.owner_user_id,run.project_id)
            AND input.event_kind='agent_run_steering_retained' AND input.payload->>'run_id'=run.run_id::text
            AND input.payload->>'conversation_id'=run.conversation_id::text
            AND (input.payload->>'input_position')::numeric=run.active_decision_position+1
          WHERE run.owner_user_id=$1::text::uuid AND run.project_id=$2::text::uuid AND run.run_id=$3::text::uuid
            AND run.status IN ('claimed','completed') AND run.fence_token=$4",
        &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &claim.fence_token],
    ).await.map_err(complete_database_error)? else { return Ok(false); };
    let position: String = row.get(3);
    let messages = client.query(
        "SELECT payload->>'author_message' FROM storyos.project_activity_event_payloads
          WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND event_kind='agent_run_steering_retained'
            AND payload->>'run_id'=$3 AND (payload->>'input_position')::numeric <= $4::text::numeric
          ORDER BY (payload->>'input_position')::numeric",
        &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &position],
    ).await.map_err(complete_database_error)?;
    let mut effective: String = row.get(0);
    for message in messages {
        effective.push('\n');
        effective.push_str(&message.get::<_, String>(0));
    }
    let assistance =
        crate::update_project_assistance::read_assistance_record(client, &claim.project_scope)
            .await
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
            .ok_or_else(|| {
                CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                    "Current guidance admission is unavailable",
                )))
            })?;
    persist_current_passage_assembly(
        client,
        &PassageContextInput {
            project_scope: &claim.project_scope,
            run_id: &claim.run_id,
            chapter_id: &row.get::<_, String>(1),
            author_message: &effective,
            receipt_id: &row.get::<_, String>(2),
            decision_position: &position,
            passage_targets: None,
            candidate_target: None,
        },
        &assistance.processing_destination_identity,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    client.execute(
        "UPDATE storyos.agent_runs SET active_decision_position=$4::text::numeric, status='claimed', lease_expires_at=COALESCE(lease_expires_at, clock_timestamp()+($5::bigint * interval '1 second')) WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND run_id=$3::text::uuid",
        &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &position, &lease_seconds],
    ).await.map_err(complete_database_error)?;
    Ok(true)
}

pub(crate) async fn inspect(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    run_id: &str,
) -> Result<
    Vec<storyos_application::AgentRunSteeringInspect>,
    storyos_application::CreateAgentRunError,
> {
    client.query(
        "SELECT input.payload->>'steering_input_id', input.payload->>'input_position', input.payload->>'author_message',
                requirement.input_snapshot_id::text, attempt.model_attempt_id::text
           FROM storyos.project_activity_event_payloads AS input
           LEFT JOIN storyos.operation_requirements AS requirement
             ON (requirement.owner_user_id,requirement.project_id)=(input.owner_user_id,input.project_id)
            AND requirement.run_id=$3::text::uuid AND requirement.requirement_role='primary'
            AND requirement.decision_position=(input.payload->>'input_position')::numeric
           LEFT JOIN storyos.model_attempts AS attempt
             ON (attempt.owner_user_id,attempt.project_id,attempt.run_id,attempt.decision_position)=
                (requirement.owner_user_id,requirement.project_id,requirement.run_id,requirement.decision_position)
            AND attempt.attempt_role='decision'
          WHERE input.owner_user_id=$1::text::uuid AND input.project_id=$2::text::uuid
            AND input.event_kind='agent_run_steering_retained' AND input.payload->>'run_id'=$3
          ORDER BY (input.payload->>'input_position')::numeric",
        &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref(), &run_id],
    ).await.map_err(|error| storyos_application::CreateAgentRunError::Unavailable(Box::new(error)))
    .map(|rows| rows.into_iter().map(|row| storyos_application::AgentRunSteeringInspect {
        steering_input_id: row.get(0), input_position: row.get(1), author_message: row.get(2),
        input_snapshot_id: row.get(3), model_attempt_id: row.get(4),
    }).collect())
}
