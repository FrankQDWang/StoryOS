//! The structure Forward evidence that Author Undo loads under the Project lock.

use storyos_application::{UndoLatestAuthorActionCommand, UndoLatestAuthorActionError};
use tokio_postgres::Client;

use crate::undo_compensation::StructureCommand;
use crate::undo_latest_author_action::undo_database_error;

pub(crate) struct ObservedStructureFrontier {
    pub sequence: u64,
    pub prior_manuscript_tree_revision: u64,
    pub resulting_manuscript_tree_revision: u64,
    pub identity: ObservedStructureIdentity,
}

pub(crate) enum ObservedStructureIdentity {
    Volume {
        volume_id: String,
    },
    VolumeDelete {
        volume_id: String,
    },
    VolumeUpdate {
        volume_id: String,
        prior_title: String,
        prior_order: u64,
    },
    Chapter {
        chapter_id: String,
    },
    ChapterDelete {
        chapter_id: String,
        prior_current_chapter_id: Option<String>,
    },
    ChapterUpdate {
        chapter_id: String,
        prior_title: String,
        prior_order: u64,
    },
}

pub(super) async fn load_structure_frontier(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    forward: StructureCommand,
    sequence: u64,
) -> Result<Option<ObservedStructureFrontier>, UndoLatestAuthorActionError> {
    let Some(row) = client
        .query_opt(
            "SELECT commit.manuscript_object_id::text, commit.resulting_revision_id::text,
                    commit.prior_revision_id::text, head.current_revision_id::text,
                    commit.prior_manuscript_tree_revision::text,
                    commit.resulting_manuscript_tree_revision::text,
                    commit.affected_volume_id::text, commit.affected_chapter_id::text,
                    payload.payload->>'prior_title', payload.payload->>'prior_order',
                    payload.payload->>'prior_current_chapter_id'
               FROM storyos.author_action_entries AS action
               JOIN storyos.authoritative_commits AS commit
                 ON (commit.owner_user_id, commit.project_id, commit.receipt_id,
                     commit.receipt_result_kind, commit.authoritative_commit_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id,
                     action.receipt_result_kind, action.authoritative_commit_id)
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (commit.owner_user_id, commit.project_id, commit.manuscript_object_id)
          LEFT JOIN storyos.project_activity_event_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id)
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?
    else {
        return Ok(None);
    };
    let (object_id, resulting_revision_id, prior_revision_id, head_revision_id) = (
        row.get::<_, Option<String>>(/*idx*/ 0),
        row.get::<_, Option<String>>(/*idx*/ 1),
        row.get::<_, Option<String>>(/*idx*/ 2),
        row.get::<_, Option<String>>(/*idx*/ 3),
    );
    let (Some(prior_tree), Some(resulting_tree)) = (
        row.get::<_, Option<String>>(/*idx*/ 4),
        row.get::<_, Option<String>>(/*idx*/ 5),
    ) else {
        return Ok(None);
    };
    let tree_only =
        object_id.is_none() && resulting_revision_id.is_none() && prior_revision_id.is_none();
    let prior_title_order = || -> Result<Option<(String, u64)>, UndoLatestAuthorActionError> {
        match (
            row.get::<_, Option<String>>(/*idx*/ 8),
            row.get::<_, Option<String>>(/*idx*/ 9),
        ) {
            (Some(title), Some(order)) => Ok(Some((title, order.parse().map_err(parse_error)?))),
            (None, _) | (_, None) => Ok(None),
        }
    };
    let identity = match (
        forward,
        row.get::<_, Option<String>>(/*idx*/ 6),
        row.get::<_, Option<String>>(/*idx*/ 7),
    ) {
        (StructureCommand::CreateVolume, Some(volume_id), None) if tree_only => {
            ObservedStructureIdentity::Volume { volume_id }
        }
        (StructureCommand::DeleteVolume, Some(volume_id), None) if tree_only => {
            ObservedStructureIdentity::VolumeDelete { volume_id }
        }
        (StructureCommand::UpdateVolume, Some(volume_id), None) if tree_only => {
            let Some((prior_title, prior_order)) = prior_title_order()? else {
                return Ok(None);
            };
            ObservedStructureIdentity::VolumeUpdate {
                volume_id,
                prior_title,
                prior_order,
            }
        }
        (StructureCommand::UpdateChapter, None, Some(chapter_id)) if tree_only => {
            let Some((prior_title, prior_order)) = prior_title_order()? else {
                return Ok(None);
            };
            ObservedStructureIdentity::ChapterUpdate {
                chapter_id,
                prior_title,
                prior_order,
            }
        }
        (StructureCommand::DeleteChapter, None, Some(chapter_id)) if tree_only => {
            ObservedStructureIdentity::ChapterDelete {
                chapter_id,
                prior_current_chapter_id: row.get(/*idx*/ 10),
            }
        }
        (StructureCommand::CreateChapter, None, Some(chapter_id))
            if object_id.as_deref() == Some(chapter_id.as_str())
                && resulting_revision_id.is_some()
                && prior_revision_id.is_none()
                && head_revision_id.is_some() =>
        {
            ObservedStructureIdentity::Chapter { chapter_id }
        }
        _ => return Ok(None),
    };
    Ok(Some(ObservedStructureFrontier {
        sequence,
        prior_manuscript_tree_revision: prior_tree.parse().map_err(parse_error)?,
        resulting_manuscript_tree_revision: resulting_tree.parse().map_err(parse_error)?,
        identity,
    }))
}

fn parse_error(error: std::num::ParseIntError) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}
