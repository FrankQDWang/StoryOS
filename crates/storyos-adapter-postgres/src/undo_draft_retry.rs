//! The reopen of a Refused Edit Draft that the undone Forward action superseded (ADR 0044).

use storyos_application::ProjectCommandError;
use storyos_core::AuthorUndoFrontierKind;

use crate::close_editor_flow_draft::{DraftReopenWrite, ObservedDraftClose, load_frontier};
use crate::undo_compensation::UndoRequest;
use crate::undo_latest_author_action::undo_database_error;

/// The Draft that the expected Author Undo Frontier superseded, which a Compensation reopens.
/// A source whose binding changed is a binding conflict.
pub(super) async fn load_source_reopen(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
) -> Result<Option<ObservedDraftClose>, ProjectCommandError> {
    let Some(source) = load_frontier(
        client,
        command,
        command.expected_author_undo_frontier_sequence,
    )
    .await?
    else {
        return Ok(None);
    };
    if source.kind != AuthorUndoFrontierKind::ReversibleDraftClose {
        return Err(ProjectCommandError::BindingConflict);
    }
    Ok(Some(source))
}

/// Reopens `source` with the Compensation Author Action at `author_action_sequence`.
pub(super) async fn persist_source_reopen(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
    source: &ObservedDraftClose,
    author_action_sequence: u64,
) -> Result<storyos_contracts::EditorFlowDraftReopened, ProjectCommandError> {
    let created_at = client
        .query_one(
            "SELECT to_char(transaction_timestamp() AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[],
        )
        .await
        .map_err(undo_database_error)?
        .get(/*idx*/ 0);
    crate::close_editor_flow_draft::persist_reopen(
        client,
        command,
        source,
        DraftReopenWrite {
            event_id: source.reopen_event_id.clone(),
            handler_receipt_id: source.handler_receipt_id.clone(),
            sequence: author_action_sequence.to_string(),
            created_at,
        },
    )
    .await
}
