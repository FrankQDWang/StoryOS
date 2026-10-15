//! The Working Target of a message with selection references (ADR 0062).

use crate::create_agent_run::agent_run_database_error as database_error;
use crate::create_agent_run::context::PassageContextInput;
use storyos_application::{CreateAgentRunError, ProjectScope, WorkingTargetAvailability};
use storyos_core::{
    AdmittedSelection, CurrentPassageAssembly, CurrentPassageAssemblyRecord, PassageContextTarget,
    SelectionReference,
};

/// Assemble the current Chapter, every Chapter block and candidate that a reference covers, and
/// the exact selected text of each reference.
pub(crate) async fn assemble(
    client: &tokio_postgres::Client,
    command: &PassageContextInput<'_>,
    source: &CurrentPassageAssembly,
    references: &[SelectionReference],
) -> Result<CurrentPassageAssemblyRecord, CreateAgentRunError> {
    let Some(source_revision) = source.chapter_revision_id.clone() else {
        return Ok(storyos_core::assemble_current_passage_context(source));
    };
    let mut targets = vec![PassageContextTarget {
        chapter_id: source.chapter_id.clone(),
        base_authoritative_revision_id: source_revision,
        manuscript_block_ids: source.proposal_target_block_ids.clone().unwrap_or_default(),
    }];
    let mut candidate_texts: Vec<(String, String)> = Vec::new();
    let mut admitted = Vec::with_capacity(references.len());
    for reference in references {
        let selection = match reference {
            SelectionReference::ChapterText {
                chapter_id,
                base_authoritative_revision_id,
                start,
                end,
            } => {
                let selection = head_blocks(client, command.project_scope, chapter_id)
                    .await?
                    .filter(|(revision_id, _)| revision_id == base_authoritative_revision_id)
                    .and_then(|(_, blocks)| {
                        let texts: Vec<_> = blocks
                            .iter()
                            .map(|block| (block.id.clone(), block.text.clone()))
                            .collect();
                        let (text, covered) =
                            storyos_core::select_chapter_text(&texts, start, end)?;
                        let open = blocks
                            .into_iter()
                            .filter(|block| !block.reserved && covered.contains(&block.id))
                            .map(|block| block.id);
                        Some((text, covered.clone(), open.collect::<Vec<_>>()))
                    });
                if let Some((_, _, open)) = &selection {
                    add_blocks(
                        &mut targets,
                        chapter_id,
                        base_authoritative_revision_id,
                        open,
                    );
                }
                selection.map(|(text, covered, _)| (text, covered))
            }
            SelectionReference::ProposalCandidateText {
                chapter_id,
                start_offset,
                end_offset,
                ..
            } => {
                let target = reference.candidate_target().expect("candidate reference");
                crate::candidate_revision_target::load(
                    client,
                    command.project_scope,
                    chapter_id,
                    &target,
                )
                .await?
                .and_then(|loaded| {
                    let text = storyos_core::select_candidate_text(
                        &loaded.text,
                        *start_offset,
                        *end_offset,
                    )?;
                    if !candidate_texts
                        .iter()
                        .any(|(revision, _)| *revision == target.revision_id)
                    {
                        candidate_texts.push((target.revision_id.clone(), loaded.text));
                    }
                    Some((text, vec![loaded.block_id]))
                })
            }
        };
        admitted.push(match selection {
            Some((text, covered)) => AdmittedSelection {
                reference: reference.clone(),
                selected_text: Some(text),
                manuscript_block_ids: covered,
            },
            None => AdmittedSelection {
                reference: reference.clone(),
                selected_text: None,
                manuscript_block_ids: Vec::new(),
            },
        });
    }
    let record = crate::passage_collection::assemble(client, command, source, &targets).await?;
    Ok(storyos_core::assemble_selection_references(
        record,
        source,
        &candidate_texts,
        admitted,
    ))
}

/// The current availability of each reference: its Chapter head or its candidate revision.
pub(crate) async fn availability(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    references: &[AdmittedSelection],
) -> Result<Vec<WorkingTargetAvailability>, CreateAgentRunError> {
    let mut current = Vec::with_capacity(references.len());
    for admitted in references {
        let (expected, head) = match &admitted.reference {
            SelectionReference::ChapterText {
                chapter_id,
                base_authoritative_revision_id,
                ..
            } => (
                base_authoritative_revision_id,
                head_blocks(client, scope, chapter_id)
                    .await?
                    .map(|(revision_id, _)| revision_id),
            ),
            SelectionReference::ProposalCandidateText {
                proposal_id,
                revision_id,
                ..
            } => (
                revision_id,
                client
                    .query_opt(
                        "SELECT current_revision_id::text FROM storyos.proposal_heads
                          WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                            AND proposal_id=$3::text::uuid",
                        &[
                            &scope.owner_user_id.as_ref(),
                            &scope.project_id.as_ref(),
                            proposal_id,
                        ],
                    )
                    .await
                    .map_err(database_error)?
                    .map(|row| row.get(/*idx*/ 0)),
            ),
        };
        current.push(match head {
            _ if admitted.selected_text.is_none() => WorkingTargetAvailability::Unavailable,
            None => WorkingTargetAvailability::Unavailable,
            Some(head) if head == *expected => WorkingTargetAvailability::Current,
            Some(head) => WorkingTargetAvailability::Superseded {
                current_revision_id: head,
            },
        });
    }
    Ok(current)
}

/// The pending candidates that the references name, once each, in reference order.
pub(crate) fn candidate_targets(
    references: &[AdmittedSelection],
) -> Vec<(String, storyos_core::ProposalCandidateTarget)> {
    let mut targets: Vec<(String, storyos_core::ProposalCandidateTarget)> = Vec::new();
    for admitted in references {
        if let Some(target) = admitted.reference.candidate_target()
            && admitted.selected_text.is_some()
            && !targets.iter().any(|(_, known)| *known == target)
        {
            targets.push((admitted.reference.chapter_id().to_owned(), target));
        }
    }
    targets
}

struct HeadBlock {
    id: String,
    text: String,
    reserved: bool,
}

/// The head revision of a Chapter and its blocks in order, with their reservation state.
async fn head_blocks(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Result<Option<(String, Vec<HeadBlock>)>, CreateAgentRunError> {
    let Some((revision_id, stored)) =
        crate::create_agent_run::context::current_chapter_payload(client, scope, chapter_id)
            .await?
    else {
        return Ok(None);
    };
    let rows = client
        .query(
            "SELECT member.manuscript_block_id::text, EXISTS (
                      SELECT 1 FROM storyos.proposal_operations AS reservation
                       WHERE reservation.owner_user_id = member.owner_user_id
                         AND reservation.project_id = member.project_id
                         AND reservation.manuscript_block_id = member.manuscript_block_id
                         AND reservation.reservation_state = 'unresolved')
               FROM storyos.manuscript_revision_members AS member
              WHERE member.owner_user_id = $1::text::uuid AND member.project_id = $2::text::uuid
                AND member.manuscript_object_id = $3::text::uuid
                AND member.revision_id = $4::text::uuid
              ORDER BY member.block_order",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id,
                &revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    let ids: Vec<String> = rows.iter().map(|row| row.get(/*idx*/ 0)).collect();
    let blocks = crate::manuscript_block::blocks_from_stored_payload(&stored, &ids);
    if blocks.len() != ids.len() {
        return Ok(None);
    }
    Ok(Some((
        revision_id,
        rows.iter()
            .zip(blocks)
            .map(|(row, block)| HeadBlock {
                id: row.get(/*idx*/ 0),
                text: block.text,
                reserved: row.get(/*idx*/ 1),
            })
            .collect(),
    )))
}

/// Add the open covered blocks to the passage target of their Chapter, once each.
fn add_blocks(
    targets: &mut Vec<PassageContextTarget>,
    chapter_id: &str,
    base_revision_id: &str,
    blocks: &[String],
) {
    let index = match targets
        .iter()
        .position(|target| target.chapter_id == chapter_id)
    {
        Some(index) => index,
        None => {
            targets.push(PassageContextTarget {
                chapter_id: chapter_id.to_owned(),
                base_authoritative_revision_id: base_revision_id.to_owned(),
                manuscript_block_ids: Vec::new(),
            });
            targets.len() - 1
        }
    };
    for block in blocks {
        if !targets[index].manuscript_block_ids.contains(block) {
            targets[index].manuscript_block_ids.push(block.clone());
        }
    }
}
