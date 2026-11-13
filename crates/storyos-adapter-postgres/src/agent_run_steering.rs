//! Steering inputs at each RunStep position (ADR 0062, ADR 0091). A steering input has the
//! position of the RunStep that reads it: the RunStep after the one that was active when the Run
//! retained it, or after the input before it. That RunStep reads the retained records of the
//! active RunStep, also its Tool results.

use crate::agent_run_work::complete_database_error;
use crate::create_agent_run::context::{PassageContextInput, persist_current_passage_assembly};
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{CurrentPassageAssemblyRecord, SelectionReference};

/// The steering input that the RunStep after the active RunStep reads.
pub(crate) struct NextInput {
    /// The Run message and each steering input up to this one, one on each line.
    pub author_message: String,
    pub receipt_id: String,
    /// The references of the Run with the references of this input (ADR 0062).
    pub references: Option<Vec<SelectionReference>>,
}

/// Reads the steering input of the RunStep after `active_position`, if any. First, the candidate
/// references and the candidate target of the requirement of `retained` move to each candidate
/// that the active RunStep revised, and then to the current revision of each pending candidate,
/// so that the next RunStep reads the current text (ADR 0062, ADR 0071, ADR 0085, ADR 0091).
pub(crate) async fn read_next(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    active_position: &str,
    retained: &mut CurrentPassageAssemblyRecord,
) -> Result<Option<NextInput>, CompleteAgentRunError> {
    move_revised_candidates(client, claim, active_position, retained).await?;
    crate::agent_run_current_candidate::follow(client, claim, retained).await?;
    let Some(row) = client
        .query_opt(
            "SELECT input.receipt_id::text, input.payload->>'input_position',
                    COALESCE(input.payload->'selection_references', 'null'::jsonb)::text,
                    run.author_message
               FROM storyos.agent_runs AS run
               JOIN storyos.project_activity_event_payloads AS input
                 ON (input.owner_user_id,input.project_id)=(run.owner_user_id,run.project_id)
                AND input.event_kind='agent_run_steering_retained' AND input.payload->>'run_id'=run.run_id::text
                AND input.payload->>'conversation_id'=run.conversation_id::text
                AND (input.payload->>'input_position')::numeric=$4::text::numeric+1
              WHERE run.owner_user_id=$1::text::uuid AND run.project_id=$2::text::uuid AND run.run_id=$3::text::uuid",
            &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &active_position],
        )
        .await
        .map_err(complete_database_error)?
    else {
        return Ok(None);
    };
    let requirement = &retained.operation_requirement;
    let position: String = row.get(/*idx*/ 1);
    let messages = client.query(
        "SELECT payload->>'author_message' FROM storyos.project_activity_event_payloads
          WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND event_kind='agent_run_steering_retained'
            AND payload->>'run_id'=$3 AND (payload->>'input_position')::numeric <= $4::text::numeric
          ORDER BY (payload->>'input_position')::numeric",
        &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &position],
    ).await.map_err(complete_database_error)?;
    let mut author_message: String = row.get(/*idx*/ 3);
    for message in messages {
        author_message.push('\n');
        author_message.push_str(&message.get::<_, String>(0));
    }
    // The references of this input join the Run references (ADR 0062).
    let mut added: Option<Vec<SelectionReference>> =
        serde_json::from_str(&row.get::<_, String>(/*idx*/ 2))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    if let Some(added) = added.as_mut() {
        crate::agent_run_current_candidate::follow_added(client, claim, added).await?;
    }
    let run_references = requirement
        .selection_references
        .as_ref()
        .map(|references| -> Vec<_> {
            references
                .iter()
                .map(|admitted| admitted.reference.clone())
                .collect()
        });
    let references = match (run_references, added) {
        (None, None) => None,
        (run, added) => Some(storyos_core::merge_steering_references(
            run.into_iter().flatten(),
            added.as_deref().unwrap_or_default(),
        )),
    };
    Ok(Some(NextInput {
        author_message,
        receipt_id: row.get(/*idx*/ 0),
        references,
    }))
}

/// The position of the last RunStep up to `position` that read a steering input, or 0.
pub(crate) async fn last_read_position(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    position: &str,
) -> Result<String, CompleteAgentRunError> {
    client
        .query_one(
            "SELECT COALESCE(max((payload->>'input_position')::numeric), 0)::text
               FROM storyos.project_activity_event_payloads
              WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                AND event_kind='agent_run_steering_retained' AND payload->>'run_id'=$3
                AND (payload->>'input_position')::numeric <= $4::text::numeric",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &position,
            ],
        )
        .await
        .map(|row| row.get(/*idx*/ 0))
        .map_err(complete_database_error)
}

/// Moves the candidate target and the references of the requirement of `retained` to each
/// candidate that a prose change or a `propose_edit` call of the RunStep at `position` revised.
async fn move_revised_candidates(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    position: &str,
    retained: &mut CurrentPassageAssemblyRecord,
) -> Result<(), CompleteAgentRunError> {
    let requirement = &mut retained.operation_requirement;
    let records = client
        .query(
            "SELECT location::text FROM storyos.model_attempts AS attempt,
                    jsonb_array_elements(COALESCE(attempt.payload #> '{decision,locations}', '[]'::jsonb)
                                         || COALESCE(attempt.payload #> '{proposed_edit,locations}', '[]'::jsonb))
                      AS location
              WHERE attempt.owner_user_id=$1::text::uuid AND attempt.project_id=$2::text::uuid
                AND attempt.run_id=$3::text::uuid AND attempt.attempt_role IN ('decision','successor')
                AND attempt.decision_position = $4::text::numeric
              ORDER BY attempt.model_attempt_id",
            &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &position],
        )
        .await
        .map_err(complete_database_error)?;
    let locations = records
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)))
        .collect::<Result<Vec<storyos_contracts::ProseChangeLocationInspect>, _>>()
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let mut grown = Vec::new();
    for location in &locations {
        if let storyos_contracts::ProseChangeLocationOutcome::Revised {
            proposal_id,
            operation_id,
            revision_id,
            prior_revision_id,
            ..
        } = &location.outcome
        {
            let prior = storyos_core::ProposalCandidateTarget {
                proposal_id: proposal_id.clone(),
                operation_id: operation_id.clone(),
                revision_id: prior_revision_id.clone(),
            };
            if let Some(target) = requirement.candidate_target.as_mut()
                && *target == prior
            {
                target.revision_id = revision_id.clone();
            }
            storyos_core::advance_candidate_references(
                requirement.selection_references.iter_mut().flatten(),
                &prior,
                revision_id,
                &location.candidate_text,
            );
            grown.push(storyos_core::GrownCandidate {
                chapter_id: &location.chapter_id,
                manuscript_block_id: &location.manuscript_block_id,
                target: storyos_core::ProposalCandidateTarget {
                    revision_id: revision_id.clone(),
                    ..prior
                },
                candidate_text: &location.candidate_text,
            });
        }
    }
    // ADR 0071: the Chapter references of the joined blocks move to their new candidates.
    requirement.selection_references = requirement
        .selection_references
        .take()
        .map(|references| storyos_core::reference_joined_candidates(references, &grown));
    Ok(())
}

/// Whether the RunStep at `position` read a steering input.
pub(crate) async fn reads_input(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    position: &str,
) -> Result<bool, CompleteAgentRunError> {
    client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.project_activity_event_payloads
                             WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                               AND event_kind='agent_run_steering_retained' AND payload->>'run_id'=$3
                               AND (payload->>'input_position')::numeric = $4::text::numeric)",
            &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &position],
        )
        .await
        .map(|row| row.get(/*idx*/ 0))
        .map_err(complete_database_error)
}

/// Opens the RunStep that reads the next steering input before the active RunStep sends its
/// request. A RunStep that read a steering input sends its request first.
pub(crate) async fn advance_before_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    active_position: &str,
    lease_seconds: i64,
) -> Result<bool, CompleteAgentRunError> {
    if reads_input(client, claim, active_position).await? {
        return Ok(false);
    }
    advance(client, claim, lease_seconds).await
}

/// Opens the RunStep that reads the next steering input after the active RunStep, with the
/// Tool results of the Run, and claims the Run again. No RunStep opens after the last RunStep of
/// the limit, and steerAgentRun retains no input for it (ADR 0091 section 5).
pub(crate) async fn advance(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    lease_seconds: i64,
) -> Result<bool, CompleteAgentRunError> {
    let Some(row) = client.query_opt(
        "SELECT run.chapter_id::text, run.active_decision_position::text,
                COALESCE(requirement.payload, 'null'::jsonb)::text
           FROM storyos.agent_runs AS run
           LEFT JOIN storyos.operation_requirements AS requirement
             ON (requirement.owner_user_id,requirement.project_id,requirement.run_id)=
                (run.owner_user_id,run.project_id,run.run_id)
            AND requirement.requirement_role='primary' AND requirement.decision_position=run.active_decision_position
          WHERE run.owner_user_id=$1::text::uuid AND run.project_id=$2::text::uuid AND run.run_id=$3::text::uuid
            AND run.status IN ('claimed','completed') AND run.fence_token=$4",
        &[&claim.project_scope.owner_user_id.as_ref(), &claim.project_scope.project_id.as_ref(), &claim.run_id, &claim.fence_token],
    ).await.map_err(complete_database_error)? else { return Ok(false); };
    let active: String = row.get(/*idx*/ 1);
    let limit = active
        .parse::<u64>()
        .map(storyos_core::RunStepLimit::at)
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    if limit == storyos_core::RunStepLimit::Last {
        return Ok(false);
    }
    let payload: serde_json::Value = serde_json::from_str(&row.get::<_, String>(/*idx*/ 2))
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let mut retained = storyos_core::decode_assembly_record(&payload).ok_or_else(|| {
        CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
            "The retained Operation Requirement is unavailable",
        )))
    })?;
    let Some(input) = read_next(client, claim, &active, &mut retained).await? else {
        return Ok(false);
    };
    let requirement = &retained.operation_requirement;
    let position = next_position(&active)?;
    let assistance =
        crate::update_project_assistance::read_assistance_record(client, &claim.project_scope)
            .await
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
            .ok_or_else(|| {
                CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                    "Current guidance admission is unavailable",
                )))
            })?;
    let results = crate::agent_run_tool_result::RunToolResults::load(client, claim).await?;
    let candidates = results.candidates();
    persist_current_passage_assembly(
        client,
        &PassageContextInput {
            project_scope: &claim.project_scope,
            run_id: &claim.run_id,
            chapter_id: &row.get::<_, String>(/*idx*/ 0),
            author_message: &input.author_message,
            receipt_id: &input.receipt_id,
            decision_position: &position,
            passage_targets: requirement.passage_targets.as_deref(),
            candidate_target: requirement.candidate_target.as_ref(),
            selection_references: input.references.as_deref(),
            tool_results: (!candidates.is_empty()).then_some(candidates.as_slice()),
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

/// The position after `position`.
pub(crate) fn next_position(position: &str) -> Result<String, CompleteAgentRunError> {
    position
        .parse::<u64>()
        .map(|position| (position + 1).to_string())
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))
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

#[cfg(test)]
#[path = "agent_run_steering_tests.rs"]
pub(crate) mod tests;
