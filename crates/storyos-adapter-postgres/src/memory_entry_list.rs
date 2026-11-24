//! The read of the current Memory Entries of a Project for the settings page (ADR 0094 section 9).
//! The source of an entry is the first source reference of its current version. A reference to a
//! record that the Project does not have now is unavailable and does not fail the read (Memory
//! contract, section 10). A flag version gives the flag with its source (ADR 0094 section 7). A
//! character entry of the background work gives its current intent result at the current Memory
//! revision (ADR 0101 section 4). The read does not read the Conversation Memory Settings. No
//! diagnostic
//! records a title, a description, or a text of an entry (ADR 0047).

use std::collections::{HashMap, HashSet};

use storyos_application::{
    MemoryEntryFlagItem, MemoryEntryIntentItem, MemoryEntryListItem, MemoryEntryReader,
    MemoryEntrySourceItem, MemoryIntentChapterItem, ProjectReadError, ProjectScope,
};
use storyos_core::{
    MemoryEntryOrigin, MemoryEntryType, MemorySourceReference, MemoryVersionFacts,
    MemoryVersionKind,
};

use super::{PostgresProjectReader, read_error, set_scope};
use crate::memory_entries::{LAST_CHANGE, TIME_FORM};

/// The rows of the current entries, the Conversations of the Project, the title of each live
/// Chapter, and the current intent results by entry, or `None` when the Project is not visible.
type Facts = (
    Vec<tokio_postgres::Row>,
    HashSet<String>,
    HashMap<String, String>,
    HashMap<String, tokio_postgres::Row>,
);

impl MemoryEntryReader for PostgresProjectReader {
    async fn read_memory_entries(
        &self,
        scope: &ProjectScope,
    ) -> Result<Option<Vec<MemoryEntryListItem>>, ProjectReadError> {
        let client = self.connect().await?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(read_error)?;
        let outcome = read_facts(&*client, scope).await;
        match &outcome {
            Ok(_) => client.batch_execute("COMMIT").await.map_err(read_error)?,
            Err(_) => {
                let _rollback = client.batch_execute("ROLLBACK").await;
            }
        }
        let Some((rows, conversations, chapters, mut intents)) = outcome? else {
            return Ok(None);
        };
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
        let mut intent = |memory_entry_id: &str| -> Result<_, ProjectReadError> {
            let Some(row) = intents.remove(memory_entry_id) else {
                return Ok(None);
            };
            let questions = serde_json::from_str(row.get(QUESTIONS))
                .map_err(|_| damaged("A Memory intent has invalid questions"))?;
            let chapter = (row.get::<_, Option<String>>(CHAPTER)).map(|chapter_id| {
                let title = chapters.get(&chapter_id).cloned();
                MemoryIntentChapterItem {
                    available: title.is_some(),
                    chapter_id,
                    title,
                }
            });
            Ok(Some(MemoryEntryIntentItem {
                text: row.get(INTENT),
                questions,
                chapter,
                recorded_at: row.get(RECORDED_AT),
            }))
        };
        rows.into_iter()
            .map(|row| {
                let flag = match row.get::<_, Option<String>>(10) {
                    Some(reason) => Some(MemoryEntryFlagItem {
                        reason,
                        source: source(row.get(11))
                            .ok_or_else(|| damaged("A Memory flag source is invalid"))?,
                        flagged_at: row.get(12),
                    }),
                    None => None,
                };
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
                    flag,
                    intent: intent(row.get(0))?,
                })
            })
            .collect::<Result<_, _>>()
            .map(Some)
    }
}

async fn read_facts(
    client: &impl crate::statement_cache::GenericClient,
    scope: &ProjectScope,
) -> Result<Option<Facts>, ProjectReadError> {
    set_scope(client, scope).await?;
    let values: [&(dyn tokio_postgres::types::ToSql + Sync); 2] =
        [&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()];
    let project = client
        .query_opt(
            "SELECT 1 FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &values,
        )
        .await
        .map_err(read_error)?;
    if project.is_none() {
        return Ok(None);
    }
    let rows = client
        .query(
            &format!(
                "SELECT memory_entry_id::text, entry_number::text, version::text, entry_type,
                        title, description, text, origin, (source_references -> 0)::text,
                        to_char({LAST_CHANGE} AT TIME ZONE 'UTC', '{TIME_FORM}'),
                        flag_reason, flag_source::text,
                        to_char(recorded_at AT TIME ZONE 'UTC', '{TIME_FORM}')
                   FROM (SELECT DISTINCT ON (memory_entry_id) *
                           FROM storyos.memory_entry_versions
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                          ORDER BY memory_entry_id, memory_revision DESC) AS latest
                  WHERE state = 'current'"
            ),
            &values,
        )
        .await
        .map_err(read_error)?;
    let conversations: HashSet<String> = client
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
    let chapters: HashMap<String, String> = client
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
    let intents = current_intents(client, &values).await?;
    Ok(Some((rows, conversations, chapters, intents)))
}

/// The columns of a row of `VERSIONS` that give the result of a version with an intent.
pub(crate) const INTENT: usize = 10;
const QUESTIONS: usize = 11;
const CHAPTER: usize = 12;
const RECORDED_AT: usize = 13;

/// Each version of the entries of the Project.
const VERSIONS: &str =
    "SELECT memory_entry_id::text, version::text, memory_revision::text, state, entry_type,
        origin = 'author_request', title, flag_reason IS NOT NULL, intent_author_entry_id::text,
        intent_author_version::text, intent, intent_questions::text, intent_chapter_id::text,
        to_char(recorded_at AT TIME ZONE 'UTC', $3)
   FROM storyos.memory_entry_versions
  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid";

/// The intent version of each entry with a current result at the current Memory revision, by the
/// rule of Core.
async fn current_intents(
    client: &impl crate::statement_cache::GenericClient,
    values: &[&(dyn tokio_postgres::types::ToSql + Sync); 2],
) -> Result<HashMap<String, tokio_postgres::Row>, ProjectReadError> {
    let head = client
        .query_opt(
            "SELECT memory_revision::text FROM storyos.project_memory_heads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            values,
        )
        .await
        .map_err(read_error)?;
    let Some(head) = head else {
        return Ok(HashMap::new());
    };
    intents_at(client, values, counter(head.get(0))?).await
}

/// The intent version of each entry with a current result at the Memory revision `revision`, by
/// the rule of Core. A Run of the author reads it at its pinned revision (ADR 0101 section 5).
pub(crate) async fn intents_at(
    client: &impl crate::statement_cache::GenericClient,
    values: &[&(dyn tokio_postgres::types::ToSql + Sync); 2],
    revision: u64,
) -> Result<HashMap<String, tokio_postgres::Row>, ProjectReadError> {
    let rows = client
        .query(VERSIONS, &[values[0], values[1], &TIME_FORM])
        .await
        .map_err(read_error)?;
    let numbers = (rows.iter())
        .map(|row| {
            let named = row.get::<_, Option<&str>>(9).map(counter).transpose();
            Ok((counter(row.get(1))?, counter(row.get(2))?, named?))
        })
        .collect::<Result<Vec<_>, ProjectReadError>>()?;
    let facts = (rows.iter().zip(&numbers))
        .map(|(row, &(version, memory_revision, named))| {
            let kind = match (row.get::<_, Option<&str>>(INTENT), row.get::<_, bool>(7)) {
                (Some(_), _) => MemoryVersionKind::Intent {
                    author: row.get::<_, Option<&str>>(8).zip(named),
                },
                (None, true) => MemoryVersionKind::Flag,
                (None, false) => MemoryVersionKind::Change,
            };
            MemoryVersionFacts {
                memory_entry_id: row.get(0),
                version,
                memory_revision,
                state: row.get(3),
                entry_type: row.get(4),
                author: row.get(5),
                title: row.get(6),
                kind,
            }
        })
        .collect::<Vec<_>>();
    let current: HashSet<(String, u64)> = (storyos_core::current_intents(&facts, revision))
        .into_iter()
        .map(|(memory_entry_id, version)| (memory_entry_id.to_owned(), version))
        .collect();
    Ok((rows.into_iter().zip(numbers))
        .map(|(row, (version, ..))| (row.get::<_, String>(0), version, row))
        .filter(|(memory_entry_id, version, _)| {
            current.contains(&(memory_entry_id.clone(), *version))
        })
        .map(|(memory_entry_id, _, row)| (memory_entry_id, row))
        .collect())
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
