use storyos_application::{AgentRunDecisionInspect, CreateAgentRunError, ProjectScope};
use storyos_contracts::{ProseChangeLocationCurrent, ProseChangeLocationOutcome};

pub(crate) async fn hydrate_locations(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    decision: &mut AgentRunDecisionInspect,
) -> Result<(), CreateAgentRunError> {
    let AgentRunDecisionInspect::ProseChange {
        decision_id,
        locations: Some(locations),
        ..
    } = decision
    else {
        return Ok(());
    };
    for location in locations {
        let ProseChangeLocationOutcome::Opened {
            proposal_id,
            operation_id,
            ..
        } = &location.outcome
        else {
            continue;
        };
        location.current = client.query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                    revision.closure, operation.resolution, operation.reservation_state
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head USING (owner_user_id, project_id, proposal_id)
               JOIN storyos.proposal_revisions AS revision ON
                 (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id) =
                 (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
               JOIN storyos.proposal_operations AS operation ON
                 (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                 (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
              WHERE proposal.owner_user_id = $1::text::uuid AND proposal.project_id = $2::text::uuid
                AND proposal.proposal_id = $3::text::uuid AND operation.operation_id = $4::text::uuid
                AND proposal.source_decision_id = $5::text::uuid AND proposal.chapter_id = $6::text::uuid
                AND operation.manuscript_block_id = $7::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref(), &proposal_id, &operation_id,
              &decision_id.as_str(), &location.chapter_id, &location.manuscript_block_id],
        ).await.map_err(super::create_agent_run::agent_run_database_error)?.map(|row| ProseChangeLocationCurrent {
            revision_id: row.get(0), generation: row.get(1), validation: row.get(2), closure: row.get(3),
            resolution: row.get(4), reservation_state: row.get(5),
        });
    }
    Ok(())
}

pub(crate) fn decode_locations(
    payload: Option<&serde_json::Value>,
) -> Result<Option<Vec<storyos_contracts::ProseChangeLocationInspect>>, CreateAgentRunError> {
    payload
        .and_then(|payload| payload.pointer("/decision/locations"))
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(super::create_agent_run::agent_run_parse_error)
}
