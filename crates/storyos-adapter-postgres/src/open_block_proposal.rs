use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{OpenBlockProposal, open_block_proposal};
use uuid::Uuid;

pub(crate) async fn open_selected_prose_change(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let target = select_open_target(client, claim, chapter_id).await?;
    let classification = open_block_proposal(&OpenBlockProposal {
        scope_matches: true,
        target_block_present: target.block_id.is_some(),
        expected_base_revision_id: target.revision_id.clone().unwrap_or_default(),
        current_base_revision_id: target.revision_id.clone(),
        conflicting_reservation: target.conflicting_reservation,
    });
    let Some(validation_result) = classification.validation_receipt_result() else {
        return Ok(None);
    };
    persist_applied_proposal(
        client,
        claim,
        chapter_id,
        decision_id,
        candidate_text,
        &target,
        validation_result,
    )
    .await
}

struct OpenTarget {
    block_id: Option<String>,
    revision_id: Option<String>,
    conflicting_reservation: bool,
}

async fn select_open_target(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
) -> Result<OpenTarget, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT member.manuscript_block_id::text, member.revision_id::text,
                    reservation.proposal_id IS NOT NULL
               FROM storyos.authoritative_heads AS head
               JOIN storyos.manuscript_revision_members AS member
                 ON (member.owner_user_id, member.project_id, member.manuscript_object_id,
                     member.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
               LEFT JOIN storyos.proposal_operations AS reservation
                 ON (reservation.owner_user_id, reservation.project_id,
                     reservation.manuscript_block_id) =
                    (member.owner_user_id, member.project_id, member.manuscript_block_id)
                AND reservation.reservation_state = 'unresolved'
              WHERE head.owner_user_id = $1::text::uuid
                AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid
              ORDER BY member.block_order",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(database_error)?;
    let mut first_live = None;
    for row in rows {
        let block_id: String = row.get(0);
        let revision_id: String = row.get(1);
        let reserved: bool = row.get(2);
        if first_live.is_none() {
            first_live = Some((block_id.clone(), revision_id.clone()));
        }
        if !reserved {
            return Ok(OpenTarget {
                block_id: Some(block_id),
                revision_id: Some(revision_id),
                conflicting_reservation: false,
            });
        }
    }
    Ok(match first_live {
        Some((block_id, revision_id)) => OpenTarget {
            block_id: Some(block_id),
            revision_id: Some(revision_id),
            conflicting_reservation: true,
        },
        None => OpenTarget {
            block_id: None,
            revision_id: None,
            conflicting_reservation: false,
        },
    })
}

async fn persist_applied_proposal(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
    target: &OpenTarget,
    validation_result: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let Some(block_id) = target.block_id.as_deref() else {
        return Ok(None);
    };
    let Some(revision_id) = target.revision_id.as_deref() else {
        return Ok(None);
    };
    let proposal_id = Uuid::now_v7().to_string();
    let proposal_revision_id = Uuid::now_v7().to_string();
    let operation_id = Uuid::now_v7().to_string();
    let validation_receipt_id = Uuid::now_v7().to_string();
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    client
        .execute(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'block_edit',
                     $4::text::uuid, $5::text::uuid, $6::text::uuid, $7::text::uuid)",
            &[
                &owner,
                &project,
                &proposal_id,
                &chapter_id,
                &block_id,
                &claim.run_id,
                &decision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                     'valid', 'open', $5, $6::text::uuid)",
            &[
                &owner,
                &project,
                &proposal_id,
                &proposal_revision_id,
                &candidate_text,
                &revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[&owner, &project, &proposal_id, &proposal_revision_id],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'pending', 'unresolved')",
            &[&owner, &project, &proposal_id, &operation_id, &block_id],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7::text::uuid, $8::text::uuid, $9, 'unresolved')",
            &[
                &owner,
                &project,
                &validation_receipt_id,
                &proposal_id,
                &proposal_revision_id,
                &validation_result,
                &revision_id,
                &block_id,
                &candidate_text,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(Some(proposal_id))
}

fn database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
