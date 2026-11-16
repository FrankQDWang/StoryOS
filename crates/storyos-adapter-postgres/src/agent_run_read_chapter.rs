//! The in-process execution of a `read_chapter` call (ADR 0095 section 1). It reads the live
//! Chapters of the Project of the Run in manuscript order, with the authoritative head of each,
//! and it settles the call with the text of the Chapter that has the number, or refuses it with
//! the reason. It reads no Proposal candidate, and it writes no Outbound Disclosure Event.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ReadableChapter, ToolCallState};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

/// The live Chapters of the Project of the Run in manuscript order, each with the text of each
/// Block of its authoritative head. The Host Tools of the manuscript read them.
pub(crate) async fn readable_chapters(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Vec<ReadableChapter>, CompleteAgentRunError> {
    Ok(crate::manuscript_search::read_live_chapter_blocks(
        client,
        &claim.project_scope,
        crate::manuscript_search::LiveChapterReadExtent::AllChapters,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
    .into_iter()
    .map(|fact| ReadableChapter {
        chapter_id: fact.chapter_id.as_ref().to_owned(),
        title: fact.title,
        revision_id: fact.head_revision_id,
        blocks: fact.blocks.into_iter().map(|block| block.text).collect(),
    })
    .collect())
}

/// Reads the Chapter with the number `chapter` for the pending call `row`, and settles or refuses
/// `row`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    chapter: i64,
) -> Result<(), CompleteAgentRunError> {
    let chapters = readable_chapters(client, claim).await?;
    let (state, refusal, result) =
        match storyos_core::admit_chapter_read(&chapters, chapter, &row.call_id) {
            Ok(result) => (ToolCallState::Settled, None, result),
            Err(refusal) => (
                ToolCallState::Refused,
                Some(refusal.label()),
                refusal.result(),
            ),
        };
    resolve(
        client,
        claim,
        row,
        Resolution {
            state,
            refusal,
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_read_chapter_tests.rs"]
pub(crate) mod tests;
