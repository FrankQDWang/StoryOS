use storyos_application::{ApplyAuthorEditCommand, AuthorEditError, ProjectScope};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditRefusal, AuthorEditUnit, CurrentOwnershipFacts,
    InlineEditDisposition, InlineInputOwner, OpenBlockProposal, classify_inline_input_owner,
    open_block_proposal,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::author_edit::author_edit_database_error;

mod compensation;
pub(crate) use compensation::{
    ObservedProposalFrontier, ProposalEditCompensation, settle_proposal_compensation,
};

#[derive(Clone, Debug)]
pub(super) struct ProposalEditContext {
    pub proposal_id: String,
    pub operation_id: Option<String>,
    pub prior_revision_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
    pub kind: String,
    pub ranges: Vec<(u32, u32)>,
    pub candidate_text: String,
    pub structured_candidate: bool,
}

pub(super) struct RoutedInlineAuthorEdit {
    pub current_body: String,
    pub author_edit_units: Vec<AuthorEditUnit>,
    pub disposition: InlineEditDisposition,
    pub proposal_context: Option<ProposalEditContext>,
}

pub(super) struct LoadedProposalHeads {
    pub ownership: CurrentOwnershipFacts,
    pub edit_body: String,
    pub context: Option<ProposalEditContext>,
}

pub(super) async fn load_chapter_proposal_heads(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    manuscript_body: String,
) -> Result<LoadedProposalHeads, AuthorEditError> {
    let structured = command
        .author_edit_units
        .iter()
        .any(|unit| unit.selection_snapshot.ordered_selection.is_some());
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
    let row_limit = if versioned || command.proposal_target.is_some() {
        None
    } else {
        structured.then(|| command.expected_proposal_head_revision_ids.len() as i64 + 1)
    };
    let rows = client
        .query(
            "SELECT head.current_revision_id::text, proposal.proposal_id::text,
                    CASE WHEN $4 THEN '' ELSE operation.candidate_text END, operation.manuscript_block_id::text,
                    revision.base_authoritative_revision_id::text, proposal.kind,
                    operation.operation_id::text, operation.resolution,
                    operation.reservation_state, operation.candidate_blocks IS NOT NULL
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
              ORDER BY head.current_revision_id, operation.operation_id LIMIT $5",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &structured,
                &row_limit,
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
                && matches!(kind.as_str(), "block_edit" | "inline_edit")
        });
        let inline_target = !structured
            && command.proposal_target.is_none()
            && kind == "inline_edit"
            && command.expected_proposal_head_revision_ids == [revision_id.clone()];
        if explicit_target || inline_target {
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
                    kind: kind.clone(),
                    ranges: Vec::new(),
                    candidate_text: candidate_text.clone(),
                    structured_candidate: row.get(9),
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
                    AuthorEditPrimitive::ReplaceStructuredSelection { .. }
                    | AuthorEditPrimitive::ReplaceSelection { .. } => false,
                })
        });
        if kind == "block_edit" && versioned && !acknowledged && !mentions_block {
            continue;
        }
        heads.push(revision_id);
    }
    heads.sort();
    heads.dedup();
    let (mut context, edit_body) = match selected {
        Some((context, candidate)) => (Some(context), Some(candidate)),
        None => (None, None),
    };
    if let Some(selected) = context.as_mut()
        && selected.kind == "inline_edit"
    {
        selected.ranges = load_inline_ranges(client, command, &selected.proposal_id).await?;
    }
    Ok(LoadedProposalHeads {
        ownership: CurrentOwnershipFacts {
            proposal_head_revision_ids: heads,
            anchor_refs: context
                .as_ref()
                .map(|selected| {
                    selected
                        .ranges
                        .iter()
                        .map(|(from, to)| format!("{from}:{to}"))
                        .collect()
                })
                .unwrap_or_default(),
            unresolved_reservation_refs: Vec::new(),
        },
        edit_body: edit_body.unwrap_or(manuscript_body),
        context,
    })
}

async fn load_inline_ranges(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    proposal_id: &str,
) -> Result<Vec<(u32, u32)>, AuthorEditError> {
    let rows = client
        .query(
            "SELECT range_from, range_to
               FROM storyos.proposal_anchors
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
              ORDER BY anchor_order",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &proposal_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    rows.into_iter()
        .map(|row| {
            let from = u32::try_from(row.get::<_, i32>(0))
                .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
            let to = u32::try_from(row.get::<_, i32>(1))
                .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
            Ok((from, to))
        })
        .collect()
}

pub(super) fn route_inline_author_edit(
    loaded: &LoadedProposalHeads,
    manuscript_body: String,
    author_edit_units: Vec<AuthorEditUnit>,
) -> Result<RoutedInlineAuthorEdit, AuthorEditRefusal> {
    let Some(context) = loaded.context.clone() else {
        return Ok(RoutedInlineAuthorEdit {
            current_body: loaded.edit_body.clone(),
            author_edit_units,
            disposition: InlineEditDisposition::Unspecified,
            proposal_context: None,
        });
    };
    if context.kind != "inline_edit" || context.ranges.is_empty() {
        return Ok(RoutedInlineAuthorEdit {
            current_body: loaded.edit_body.clone(),
            author_edit_units,
            disposition: InlineEditDisposition::Unspecified,
            proposal_context: Some(context),
        });
    }
    let Some((from, to)) = first_replace_range(&author_edit_units) else {
        return Err(AuthorEditRefusal::UnsupportedIntentShape);
    };
    if context.operation_id.is_some() {
        if context.structured_candidate {
            return Err(AuthorEditRefusal::UnsupportedIntentShape);
        }
        let candidate_to = u32::try_from(context.candidate_text.encode_utf16().count())
            .map_err(|_| AuthorEditRefusal::InvalidSelection)?;
        if classify_inline_input_owner(&[(0, candidate_to)], from, to) != InlineInputOwner::Proposal
        {
            return Err(AuthorEditRefusal::InvalidSelection);
        }
        return Ok(RoutedInlineAuthorEdit {
            current_body: loaded.edit_body.clone(),
            author_edit_units,
            disposition: InlineEditDisposition::Unspecified,
            proposal_context: Some(context),
        });
    }
    match classify_inline_input_owner(&context.ranges, from, to) {
        InlineInputOwner::Proposal => {
            if context.structured_candidate {
                return Err(AuthorEditRefusal::UnsupportedIntentShape);
            }
            let Some((anchor_from, _)) = context.ranges.first().copied() else {
                return Err(AuthorEditRefusal::UnsupportedIntentShape);
            };
            Ok(RoutedInlineAuthorEdit {
                current_body: loaded.edit_body.clone(),
                author_edit_units: remap_units(&author_edit_units, anchor_from)?,
                disposition: InlineEditDisposition::Unspecified,
                proposal_context: Some(context),
            })
        }
        InlineInputOwner::Authoritative => Ok(RoutedInlineAuthorEdit {
            current_body: manuscript_body,
            author_edit_units,
            disposition: InlineEditDisposition::AuthoritativeDespiteReservation,
            proposal_context: Some(context),
        }),
        InlineInputOwner::Mixed => Err(AuthorEditRefusal::UnsupportedIntentShape),
    }
}

fn first_replace_range(units: &[AuthorEditUnit]) -> Option<(u32, u32)> {
    let unit = units.first()?;
    match unit.normalized_primitives.first()? {
        AuthorEditPrimitive::ReplaceSelection { from, to, .. } => Some((*from, *to)),
        AuthorEditPrimitive::ReplaceBlockSelection { from, to, .. } => Some((*from, *to)),
        _ => None,
    }
}

fn remap_units(
    units: &[AuthorEditUnit],
    anchor_from: u32,
) -> Result<Vec<AuthorEditUnit>, AuthorEditRefusal> {
    units
        .iter()
        .map(|unit| {
            let remapped = unit
                .normalized_primitives
                .iter()
                .map(|primitive| match primitive {
                    AuthorEditPrimitive::ReplaceSelection { from, to, text } => {
                        Ok(AuthorEditPrimitive::ReplaceSelection {
                            from: from
                                .checked_sub(anchor_from)
                                .ok_or(AuthorEditRefusal::InvalidSelection)?,
                            to: to
                                .checked_sub(anchor_from)
                                .ok_or(AuthorEditRefusal::InvalidSelection)?,
                            text: text.clone(),
                        })
                    }
                    _ => Err(AuthorEditRefusal::UnsupportedIntentShape),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let from = unit
                .selection_snapshot
                .from
                .checked_sub(anchor_from)
                .ok_or(AuthorEditRefusal::InvalidSelection)?;
            let to = unit
                .selection_snapshot
                .to
                .checked_sub(anchor_from)
                .ok_or(AuthorEditRefusal::InvalidSelection)?;
            Ok(AuthorEditUnit {
                normalized_primitives: remapped,
                selection_snapshot: storyos_core::SelectionSnapshot {
                    ordered_selection: None,
                    coordinate_profile: unit.selection_snapshot.coordinate_profile.clone(),
                    from,
                    to,
                },
            })
        })
        .collect()
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
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id,
                candidate_blocks)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                     $5, 'open', (SELECT CASE WHEN proposal.manuscript_block_id=$9::text::uuid
                       THEN $6 ELSE prior.candidate_text END
                       FROM storyos.proposals AS proposal
                       JOIN storyos.proposal_revisions AS prior
                         USING (owner_user_id, project_id, proposal_id)
                      WHERE proposal.owner_user_id=$1::text::uuid AND proposal.project_id=$2::text::uuid
                        AND proposal.proposal_id=$3::text::uuid AND prior.revision_id=$8::text::uuid),
                     $7::text::uuid, $8::text::uuid,
                     (SELECT candidate_blocks FROM storyos.proposal_revisions
                       WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                         AND proposal_id=$3::text::uuid AND revision_id=$8::text::uuid))",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &revision_id,
                &validation,
                &candidate_text,
                &context.base_authoritative_revision_id,
                &context.prior_revision_id,
                &context.manuscript_block_id,
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
    let operation_updates = client
        .execute(
            "UPDATE storyos.proposal_operations
                SET candidate_text = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
                AND manuscript_block_id = $5::text::uuid
                AND ($6::text IS NULL OR operation_id = $6::text::uuid)
                AND resolution = 'pending'",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &candidate_text,
                &context.manuscript_block_id,
                &context.operation_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if context.operation_id.is_some() && operation_updates != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
    Ok(revision_id)
}
