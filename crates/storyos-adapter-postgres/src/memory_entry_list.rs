//! The read of the current Memory Entries of a Project for the settings page (ADR 0094 section 9).
//! The source of an entry is the first source reference of its current version. A reference to a
//! record that the Project does not have now is unavailable and does not fail the read (Memory
//! contract, section 10). The read does not read the Conversation Memory Settings. No diagnostic
//! records a title, a description, or a text of an entry (ADR 0047).

use std::collections::{HashMap, HashSet};

use storyos_application::{
    MemoryEntryListItem, MemoryEntryReader, MemoryEntrySourceItem, ProjectReadError, ProjectScope,
};
use storyos_core::{MemoryEntryOrigin, MemoryEntryType, MemorySourceReference};

use super::{PostgresProjectReader, read_error, set_scope};

impl MemoryEntryReader for PostgresProjectReader {
    async fn read_memory_entries(
        &self,
        scope: &ProjectScope,
    ) -> Result<Option<Vec<MemoryEntryListItem>>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let values: [&(dyn tokio_postgres::types::ToSql + Sync); 2] =
            [&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()];
        let project = transaction
            .query_opt(
                "SELECT 1 FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &values,
            )
            .await
            .map_err(read_error)?;
        if project.is_none() {
            transaction.commit().await.map_err(read_error)?;
            return Ok(None);
        }
        let rows = transaction
            .query(
                "SELECT memory_entry_id::text, entry_number::text, version::text, entry_type,
                        title, description, text, origin, (source_references -> 0)::text,
                        to_char(recorded_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                   FROM (SELECT DISTINCT ON (memory_entry_id) *
                           FROM storyos.memory_entry_versions
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                          ORDER BY memory_entry_id, memory_revision DESC) AS latest
                  WHERE state = 'current'",
                &values,
            )
            .await
            .map_err(read_error)?;
        let conversations: HashSet<String> = transaction
            .query(
                "SELECT conversation_id::text FROM storyos.project_conversations
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &values,
            )
            .await
            .map_err(read_error)?
            .iter()
            .map(|row| row.get(0))
            .collect();
        // A Chapter is in the manuscript tree when neither the Chapter nor its Volume is removed.
        let chapters: HashMap<String, String> = transaction
            .query(
                "SELECT chapter.manuscript_object_id::text, chapter.title
                   FROM storyos.manuscript_objects AS chapter
                   JOIN storyos.manuscript_objects AS volume
                     ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                        (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
                  WHERE chapter.owner_user_id = $1::text::uuid
                    AND chapter.project_id = $2::text::uuid
                    AND chapter.object_kind = 'chapter'
                    AND volume.object_kind = 'volume'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE removal.owner_user_id = chapter.owner_user_id
                         AND removal.project_id = chapter.project_id
                         AND removal.chapter_id = chapter.manuscript_object_id)
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.volume_removal_decisions AS removal
                       WHERE removal.owner_user_id = volume.owner_user_id
                         AND removal.project_id = volume.project_id
                         AND removal.volume_id = volume.manuscript_object_id)",
                &values,
            )
            .await
            .map_err(read_error)?
            .iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect();
        transaction.commit().await.map_err(read_error)?;
        let source =
            |reference: Option<String>| match serde_json::from_str::<MemorySourceReference>(
                &reference?,
            )
            .ok()?
            {
                MemorySourceReference::Conversation {
                    conversation_id, ..
                } => Some(MemoryEntrySourceItem::Conversation {
                    available: conversations.contains(&conversation_id),
                    conversation_id,
                }),
                MemorySourceReference::Chapter { chapter_id } => {
                    let title = chapters.get(&chapter_id).cloned();
                    Some(MemoryEntrySourceItem::Chapter {
                        available: title.is_some(),
                        chapter_id,
                        title,
                    })
                }
            };
        rows.into_iter()
            .map(|row| {
                Ok(MemoryEntryListItem {
                    memory_entry_id: row.get(0),
                    number: counter(row.get(1))?,
                    version: counter(row.get(2))?,
                    entry_type: MemoryEntryType::parse(row.get(3))
                        .ok_or_else(|| damaged("A Memory Entry type is invalid"))?,
                    title: row.get(4),
                    description: row.get(5),
                    text: row.get(6),
                    origin: MemoryEntryOrigin::parse(row.get(7))
                        .ok_or_else(|| damaged("A Memory Entry origin is invalid"))?,
                    source: source(row.get(8)),
                    changed_at: row.get(9),
                })
            })
            .collect::<Result<_, _>>()
            .map(Some)
    }
}

fn damaged(message: &'static str) -> ProjectReadError {
    ProjectReadError::unavailable(std::io::Error::other(message))
}

fn counter(value: &str) -> Result<u64, ProjectReadError> {
    value
        .parse()
        .map_err(|_| damaged("A Memory counter is not a 64-bit number"))
}

#[cfg(test)]
#[path = "memory_entry_list_tests.rs"]
mod tests;
