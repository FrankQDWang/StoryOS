use storyos_application::{ApplyAuthorEditCommand, AuthorEditError, ProjectScope};
use storyos_core::{CurrentOwnershipFacts, OpenBlockProposal, open_block_proposal};
use tokio_postgres::Client;
use uuid::Uuid;

use super::author_edit::author_edit_database_error;

#[derive(Debug)]
pub(super) struct ProposalEditContext {
    pub proposal_id: String,
    pub prior_revision_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
}

pub(super) struct LoadedProposalHeads {
    pub ownership: CurrentOwnershipFacts,
    pub edit_body: String,
    pub context: Option<ProposalEditContext>,
}

pub(super) struct ObservedProposalFrontier {
    pub sequence: u64,
    pub chapter_id: String,
    pub proposal_id: String,
    pub current_revision_id: String,
    pub restored_candidate_text: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
}

pub(super) async fn load_chapter_proposal_heads(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    manuscript_body: String,
) -> Result<LoadedProposalHeads, AuthorEditError> {
    let rows = client
        .query(
            "SELECT head.current_revision_id::text, proposal.proposal_id::text,
                    revision.candidate_text, proposal.manuscript_block_id::text,
                    revision.base_authoritative_revision_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.chapter_id = $3::text::uuid
              ORDER BY head.current_revision_id",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let mut heads = Vec::new();
    let mut selected = None;
    for row in rows {
        let revision_id: String = row.get(0);
        if command.expected_proposal_head_revision_ids == [revision_id.clone()] {
            selected = Some((
                ProposalEditContext {
                    proposal_id: row.get(1),
                    prior_revision_id: revision_id.clone(),
                    manuscript_block_id: row.get(3),
                    base_authoritative_revision_id: row.get(4),
                },
                row.get::<_, String>(2),
            ));
        }
        heads.push(revision_id);
    }
    let (context, edit_body) = match selected {
        Some((context, candidate)) => (Some(context), Some(candidate)),
        None => (None, None),
    };
    Ok(LoadedProposalHeads {
        ownership: CurrentOwnershipFacts {
            proposal_head_revision_ids: heads,
            anchor_refs: Vec::new(),
            unresolved_reservation_refs: Vec::new(),
        },
        edit_body: edit_body.unwrap_or(manuscript_body),
        context,
    })
}

pub(super) async fn append_proposal_revision(
    client: &Client,
    scope: &ProjectScope,
    context: &ProposalEditContext,
    current_authoritative_revision_id: &str,
    candidate_text: &str,
) -> Result<String, AuthorEditError> {
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    let facts = client
        .query_one(
            "SELECT EXISTS (
                    SELECT 1
                      FROM storyos.authoritative_heads AS head
                      JOIN storyos.manuscript_revision_members AS member
                        ON (member.owner_user_id, member.project_id,
                            member.manuscript_object_id, member.revision_id) =
                           (head.owner_user_id, head.project_id,
                            head.manuscript_object_id, head.current_revision_id)
                     WHERE head.owner_user_id = $1::text::uuid
                       AND head.project_id = $2::text::uuid
                       AND member.manuscript_block_id = $3::text::uuid
                  ),
                  EXISTS (
                    SELECT 1
                      FROM storyos.proposal_operations AS reservation
                     WHERE reservation.owner_user_id = $1::text::uuid
                       AND reservation.project_id = $2::text::uuid
                       AND reservation.manuscript_block_id = $3::text::uuid
                       AND reservation.reservation_state = 'unresolved'
                       AND reservation.proposal_id <> $4::text::uuid
                  )",
            &[
                &owner,
                &project,
                &context.manuscript_block_id,
                &context.proposal_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let classification = open_block_proposal(&OpenBlockProposal {
        scope_matches: true,
        target_block_present: facts.get(0),
        expected_base_revision_id: context.base_authoritative_revision_id.clone(),
        current_base_revision_id: Some(current_authoritative_revision_id.to_owned()),
        conflicting_reservation: facts.get(1),
    });
    let (validation, receipt_result) = match classification {
        storyos_core::OpenBlockProposalResult::Applied => ("valid", "valid"),
        storyos_core::OpenBlockProposalResult::Conflicted { .. } => ("conflicted", "conflicted"),
        storyos_core::OpenBlockProposalResult::Refused { .. } => {
            return Err(AuthorEditError::BindingConflict);
        }
    };
    let revision_id = Uuid::now_v7().to_string();
    let validation_receipt_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                     $5, 'open', $6, $7::text::uuid, $8::text::uuid)",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &revision_id,
                &validation,
                &candidate_text,
                &context.base_authoritative_revision_id,
                &context.prior_revision_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.proposal_heads
                SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &revision_id,
                &context.prior_revision_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if head_updates != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
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
                &context.proposal_id,
                &revision_id,
                &receipt_result,
                &context.base_authoritative_revision_id,
                &context.manuscript_block_id,
                &candidate_text,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    Ok(revision_id)
}

pub(super) async fn load_proposal_frontier(
    client: &Client,
    scope: &ProjectScope,
    sequence: u64,
) -> Result<Option<ObservedProposalFrontier>, AuthorEditError> {
    let row = client
        .query_opt(
            "SELECT proposal.chapter_id::text, proposal.proposal_id::text,
                    head.current_revision_id::text, parent.candidate_text,
                    proposal.manuscript_block_id::text,
                    revision.base_authoritative_revision_id::text
               FROM storyos.author_action_entries AS action
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
                    (receipt.owner_user_id, receipt.project_id,
                     receipt.proposal_revision_ids[1])
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id) =
                    (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id)
               JOIN storyos.proposals AS proposal
                 ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                    (revision.owner_user_id, revision.project_id, revision.proposal_id)
               JOIN storyos.proposal_revisions AS parent
                 ON (parent.owner_user_id, parent.project_id, parent.proposal_id,
                     parent.revision_id) =
                    (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.parent_revision_id)
              LEFT JOIN storyos.author_action_entries AS compensation
                ON compensation.owner_user_id = action.owner_user_id
               AND compensation.project_id = action.project_id
               AND compensation.disposition = 'compensation'
               AND compensation.compensated_source_sequence = action.author_action_sequence
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric
                AND action.disposition = 'forward'
                AND receipt.result_kind = 'proposal_revised'
                AND compensation.author_action_sequence IS NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    Ok(row.map(|row| ObservedProposalFrontier {
        sequence,
        chapter_id: row.get(0),
        proposal_id: row.get(1),
        current_revision_id: row.get(2),
        restored_candidate_text: row.get(3),
        manuscript_block_id: row.get(4),
        base_authoritative_revision_id: row.get(5),
    }))
}
