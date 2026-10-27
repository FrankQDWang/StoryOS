use storyos_application::{ApplyAuthorEditCommand, AuthorEditError};
use storyos_core::{AuthorEditPrimitive, CurrentOwnershipFacts};
use tokio_postgres::Client;

use super::author_edit::author_edit_database_error;

mod compensation;
mod revision;
pub(crate) use compensation::{
    ObservedProposalFrontier, ProposalEditCompensation, decode_proposal_compensation,
    proposal_receipt_payload, settle_proposal_compensation,
};
pub(super) use revision::append_proposal_revision_as;

#[derive(Clone, Debug)]
pub(super) struct ProposalEditContext {
    pub proposal_id: String,
    pub operation_id: Option<String>,
    pub prior_revision_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
}

pub(super) struct LoadedProposalHeads {
    pub ownership: CurrentOwnershipFacts,
    pub candidate_body: Option<String>,
    pub context: Option<ProposalEditContext>,
}

pub(super) async fn load_chapter_proposal_heads(
    client: &Client,
    command: &ApplyAuthorEditCommand,
) -> Result<LoadedProposalHeads, AuthorEditError> {
    let versioned = command.author_edit_units.iter().any(|unit| {
        unit.normalized_primitives.iter().any(|primitive| {
            matches!(
                primitive,
                AuthorEditPrimitive::ReplaceBlockSelection { .. }
                    | AuthorEditPrimitive::SplitBlock { .. }
                    | AuthorEditPrimitive::JoinBlocks { .. }
                    | AuthorEditPrimitive::MoveBlock { .. }
                    | AuthorEditPrimitive::RetypeBlock { .. }
            )
        })
    });
    let rows = client
        .query(
            "SELECT head.current_revision_id::text, proposal.proposal_id::text,
                    operation.candidate_text, operation.manuscript_block_id::text,
                    revision.base_authoritative_revision_id::text, proposal.kind,
                    operation.operation_id::text, operation.resolution,
                    operation.reservation_state
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
              JOIN storyos.proposal_operations AS operation
                ON (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                   (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.chapter_id = $3::text::uuid
                AND revision.closure = 'open'
                AND operation.resolution = 'pending'
                AND operation.reservation_state = 'unresolved'
              ORDER BY head.current_revision_id, operation.operation_id",
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
        let proposal_id: String = row.get(1);
        let manuscript_block_id: String = row.get(3);
        let kind: String = row.get(5);
        let explicit_target = command.proposal_target.as_ref().is_some_and(|target| {
            target.proposal_id == proposal_id
                && target.revision_id == revision_id
                && target.manuscript_block_id == manuscript_block_id
                && row.get::<_, Option<String>>(6).as_deref() == Some(target.operation_id.as_str())
                && row.get::<_, Option<String>>(7).as_deref() == Some("pending")
                && row.get::<_, Option<String>>(8).as_deref() == Some("unresolved")
                && kind == "block_edit"
        });
        if explicit_target {
            let candidate_text: String = row.get(2);
            selected = Some((
                ProposalEditContext {
                    proposal_id: proposal_id.clone(),
                    operation_id: command
                        .proposal_target
                        .as_ref()
                        .map(|target| target.operation_id.clone()),
                    prior_revision_id: revision_id.clone(),
                    manuscript_block_id: manuscript_block_id.clone(),
                    base_authoritative_revision_id: row.get(4),
                },
                candidate_text,
            ));
        }
        let acknowledged = command
            .expected_proposal_head_revision_ids
            .iter()
            .any(|head| head == &revision_id)
            || command.proposal_target.as_ref().is_some_and(|target| {
                target.proposal_id == proposal_id && target.revision_id == revision_id
            });
        let mentions_block = command.author_edit_units.iter().any(|unit| {
            unit.normalized_primitives
                .iter()
                .any(|primitive| match primitive {
                    AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id: edited_block_id,
                        ..
                    }
                    | AuthorEditPrimitive::MoveBlock {
                        manuscript_block_id: edited_block_id,
                        ..
                    }
                    | AuthorEditPrimitive::RetypeBlock {
                        manuscript_block_id: edited_block_id,
                        ..
                    } => edited_block_id == &manuscript_block_id,
                    AuthorEditPrimitive::SplitBlock {
                        manuscript_block_id: edited_block_id,
                        new_manuscript_block_id,
                        ..
                    } => {
                        edited_block_id == &manuscript_block_id
                            || new_manuscript_block_id == &manuscript_block_id
                    }
                    AuthorEditPrimitive::JoinBlocks {
                        left_manuscript_block_id,
                        right_manuscript_block_id,
                    } => {
                        left_manuscript_block_id == &manuscript_block_id
                            || right_manuscript_block_id == &manuscript_block_id
                    }
                    AuthorEditPrimitive::ReplaceSelection { .. } => false,
                })
        });
        if kind == "block_edit" && versioned && !acknowledged && !mentions_block {
            continue;
        }
        heads.push(revision_id);
    }
    heads.sort();
    heads.dedup();
    let (context, candidate_body) = match selected {
        Some((context, candidate)) => (Some(context), Some(candidate)),
        None => (None, None),
    };
    Ok(LoadedProposalHeads {
        ownership: CurrentOwnershipFacts {
            proposal_head_revision_ids: heads,
            unresolved_reservation_refs: Vec::new(),
        },
        candidate_body,
        context,
    })
}
