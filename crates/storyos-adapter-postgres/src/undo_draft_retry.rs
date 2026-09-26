use storyos_application::{UndoLatestAuthorActionCommand, UndoLatestAuthorActionError};
use storyos_core::AuthorUndoFrontierKind;
use uuid::Uuid;

use crate::undo_draft_close::{DraftReopenWrite, load_frontier};
use crate::undo_latest_author_action::undo_database_error;

pub(super) async fn persist_reopen(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<Option<storyos_contracts::EditorFlowDraftReopened>, UndoLatestAuthorActionError> {
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
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    let scope = &command.project_scope;
    let row = client.query_one("SELECT author_action_sequence::text,
        to_char(transaction_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
        FROM storyos.scope_counters WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid",
        &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref()]).await.map_err(undo_database_error)?;
    let event = crate::undo_draft_close::persist_reopen(
        client,
        command,
        &source,
        DraftReopenWrite {
            event_id: Uuid::now_v7().to_string(),
            handler_receipt_id: Uuid::now_v7().to_string(),
            sequence: row.get(0),
            created_at: row.get(1),
        },
    )
    .await?;
    Ok(Some(event))
}

pub(super) async fn read_reopen(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    receipt_id: &str,
) -> Result<Option<storyos_contracts::EditorFlowDraftReopened>, UndoLatestAuthorActionError> {
    let scope = &command.project_scope;
    let row = client.query_opt("SELECT event.event_id::text FROM storyos.domain_receipts AS receipt
      JOIN storyos.draft_reopen_receipts AS handler ON (handler.owner_user_id,handler.project_id,handler.author_undo_receipt_id)=
        (receipt.owner_user_id,receipt.project_id,receipt.receipt_id)
      JOIN storyos.draft_reopen_events AS event ON (event.owner_user_id,event.project_id,event.handler_receipt_id)=
        (handler.owner_user_id,handler.project_id,handler.receipt_id)
      JOIN storyos.draft_close_events AS source ON (source.owner_user_id,source.project_id,source.event_id)=
        (event.owner_user_id,event.project_id,event.source_close_event_id)
      WHERE receipt.owner_user_id=$1::text::uuid AND receipt.project_id=$2::text::uuid AND receipt.receipt_id=$3::text::uuid
        AND receipt.result_kind='authoritative_applied' AND source.close_reason='superseded'
        AND receipt.draft_artifact_refs=ARRAY[event.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]",
      &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref(),&receipt_id]).await.map_err(undo_database_error)?;
    match row {
        Some(row) => crate::undo_draft_close::read_event(client, scope, &row.get::<_, String>(0))
            .await
            .map(Some),
        None => Ok(None),
    }
}
