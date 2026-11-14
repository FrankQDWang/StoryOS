//! The current head of each Chapter that a `chapter_text` reference of a Run names. When the
//! head moves during the Run, the next RunStep reads the head (ADR 0085).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ChapterHeadPlace, CurrentPassageAssemblyRecord, SelectionReference};

use crate::agent_run_work::complete_database_error;

/// Moves each Chapter text reference of the requirement of `retained` to the head of its
/// Chapter, and drops a reference with no covered text in the head. A reference of a removed
/// Chapter stays at its base, so the Context Assembly finds it unavailable as before.
pub(crate) async fn follow(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    retained: &mut CurrentPassageAssemblyRecord,
) -> Result<(), CompleteAgentRunError> {
    let Some(references) = retained.operation_requirement.selection_references.as_mut() else {
        return Ok(());
    };
    let mut followed = Vec::with_capacity(references.len());
    for mut admitted in references.drain(..) {
        match place(client, claim, &admitted.reference).await? {
            Some(ChapterHeadPlace::Moved(reference)) => {
                admitted.reference = reference;
                followed.push(admitted);
            }
            Some(ChapterHeadPlace::Dropped) => {}
            None => followed.push(admitted),
        }
    }
    *references = followed;
    Ok(())
}

/// Moves each Chapter text reference of a steering input to the head of its Chapter, as
/// `follow` does for the references of the Run.
pub(crate) async fn follow_added(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    added: &mut Vec<SelectionReference>,
) -> Result<(), CompleteAgentRunError> {
    let mut followed = Vec::with_capacity(added.len());
    for reference in added.drain(..) {
        match place(client, claim, &reference).await? {
            Some(ChapterHeadPlace::Moved(moved)) => followed.push(moved),
            Some(ChapterHeadPlace::Dropped) => {}
            None => followed.push(reference),
        }
    }
    *added = followed;
    Ok(())
}

/// The place of `reference` in the head of its Chapter. `None` when the reference stays: it is
/// not a Chapter text reference, its Chapter is removed, or a revision has no readable blocks.
async fn place(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    reference: &SelectionReference,
) -> Result<Option<ChapterHeadPlace>, CompleteAgentRunError> {
    let SelectionReference::ChapterText {
        chapter_id,
        base_authoritative_revision_id,
        ..
    } = reference
    else {
        return Ok(None);
    };
    let scope = &claim.project_scope;
    let Some((head_revision_id, head_body)) =
        crate::create_agent_run::context::current_chapter_payload(client, scope, chapter_id)
            .await
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
    else {
        return Ok(None);
    };
    if head_revision_id == *base_authoritative_revision_id {
        return Ok(None);
    }
    // A Revision does not change after it is written, so its base text is still readable.
    let Some(base_body) = client
        .query_opt(
            "SELECT convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.authoritative_revisions AS revision
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
              WHERE revision.owner_user_id = $1::text::uuid AND revision.project_id = $2::text::uuid
                AND revision.manuscript_object_id = $3::text::uuid
                AND revision.revision_id = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id,
                &base_authoritative_revision_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .map(|row| row.get::<_, String>(/*idx*/ 0))
    else {
        return Ok(None);
    };
    let base = revision_blocks(
        client,
        claim,
        chapter_id,
        base_authoritative_revision_id,
        &base_body,
    )
    .await?;
    let head = revision_blocks(client, claim, chapter_id, &head_revision_id, &head_body).await?;
    if base.is_empty() || head.is_empty() {
        return Ok(None);
    }
    Ok(storyos_core::follow_chapter_head(
        reference,
        &base,
        &head_revision_id,
        &head,
    ))
}

/// The `(block id, text)` pairs of the Revision `revision_id` with the stored `body`, in order.
async fn revision_blocks(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    revision_id: &str,
    body: &str,
) -> Result<Vec<(String, String)>, CompleteAgentRunError> {
    crate::manuscript_block::load_revision_blocks(
        client,
        claim.project_scope.owner_user_id.as_ref(),
        claim.project_scope.project_id.as_ref(),
        chapter_id,
        revision_id,
        body,
    )
    .await
    .map(|blocks| {
        blocks
            .into_iter()
            .map(|block| (block.manuscript_block_id, block.text))
            .collect()
    })
    .map_err(complete_database_error)
}

#[cfg(test)]
#[path = "agent_run_current_chapter_tests.rs"]
mod tests;
