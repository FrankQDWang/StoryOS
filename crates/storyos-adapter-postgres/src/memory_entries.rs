//! The reads of Agent Memory for a Run (ADR 0094 sections 3, 4, and 8): the pin of the Memory
//! revision, the current Memory Entries at a revision, the read of one entry, and the time of the
//! last read of each entry. No diagnostic records a title, a description, or a text of an entry
//! (ADR 0047).

use storyos_application::ProjectScope;
use storyos_core::{
    CurrentMemoryEntry, LastMemoryVersion, MemoryDeleteMark, MemoryEntryRead, MemoryEntryState,
    MemoryEntryType, MemoryFlagSource, MemoryIndexEntry, MemoryReadFlag, MemorySourceReference,
    MemoryVersionWrite,
};

fn invalid(message: &'static str) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message))
}

fn numeric(value: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    value
        .parse()
        .map_err(|_| invalid("A Memory counter is not a 64-bit number"))
}

/// The Memory revision that the Run pins: the revision of the Project head when the captured
/// Conversation Memory Settings of the Run have `use_enabled` (`storyos_core::memory_revision_pin`).
pub(crate) async fn memory_revision_pin(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<u64>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_one(
            "SELECT COALESCE(settings.use_enabled, FALSE),
                    COALESCE((SELECT head.memory_revision
                                FROM storyos.project_memory_heads AS head
                               WHERE head.owner_user_id = run.owner_user_id
                                 AND head.project_id = run.project_id), 0)::text
               FROM storyos.agent_runs AS run
               LEFT JOIN storyos.conversation_memory_settings AS settings
                 ON (settings.owner_user_id, settings.project_id, settings.conversation_id,
                     settings.memory_settings_revision) =
                    (run.owner_user_id, run.project_id, run.conversation_id,
                     run.memory_settings_revision)
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await?;
    Ok(storyos_core::memory_revision_pin(
        row.get(0),
        numeric(row.get(1))?,
    ))
}

/// The current Memory Entries of the Project at `memory_revision`: for each entry, the version
/// with the highest Memory revision that is not above it, when its state is `current`. The entry
/// is flagged when that version is a flag version (ADR 0094 section 7).
pub(crate) async fn read_memory_index(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    memory_revision: u64,
) -> Result<Vec<MemoryIndexEntry>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT entry_number::text, entry_type, title, description, flag_reason IS NOT NULL
               FROM (SELECT DISTINCT ON (memory_entry_id)
                            entry_number, state, entry_type, title, description, flag_reason
                       FROM storyos.memory_entry_versions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND memory_revision <= $3::text::numeric
                      ORDER BY memory_entry_id, memory_revision DESC) AS latest
              WHERE state = 'current'",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &memory_revision.to_string(),
            ],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok(MemoryIndexEntry {
                number: numeric(row.get(0))?,
                entry_type: MemoryEntryType::parse(row.get(1))
                    .ok_or_else(|| invalid("A Memory Entry type is invalid"))?,
                title: row.get(2),
                description: row.get(3),
                flagged: row.get(4),
            })
        })
        .collect()
}

/// The Memory revision of the Project head, with no lock. A Project with no head has the revision
/// 0.
pub(crate) async fn memory_head(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT memory_revision::text FROM storyos.project_memory_heads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    row.map_or(Ok(0), |row| numeric(row.get(0)))
}

/// The marks of the entries that the author deleted, newest first (ADR 0094 section 8). A mark is
/// the last version of an entry with the state `deleted`. Its cause is a delete by the author. A
/// capture and the consolidation pass of S7-D4b read the marks. The read takes no lock.
pub(crate) async fn read_delete_marks(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<Vec<MemoryDeleteMark>, Box<dyn std::error::Error + Send + Sync>> {
    let causes = storyos_core::MEMORY_DELETE_MARK_CAUSES;
    let rows = client
        .query(
            "SELECT entry_type, title, description
               FROM (SELECT DISTINCT ON (memory_entry_id)
                            memory_revision, state, entry_type, title, description, cause
                       FROM storyos.memory_entry_versions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                      ORDER BY memory_entry_id, memory_revision DESC) AS latest
              WHERE state = 'deleted' AND cause->>'kind' = ANY($3::text[])
              ORDER BY memory_revision DESC",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &causes.as_slice(),
            ],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok(MemoryDeleteMark {
                entry_type: MemoryEntryType::parse(row.get(0))
                    .ok_or_else(|| invalid("A Memory Entry type is invalid"))?,
                title: row.get(1),
                description: row.get(2),
            })
        })
        .collect()
}

/// The Memory of the Project now: the Memory revision, the next entry number, and the last
/// version of each current entry. A Project with no head has the revision 0 and the number 1.
pub(crate) async fn current_memory(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<(u64, u64, Vec<CurrentMemoryEntry>), Box<dyn std::error::Error + Send + Sync>> {
    let values: [&(dyn tokio_postgres::types::ToSql + Sync); 2] =
        [&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()];
    let head = client
        .query_opt(
            "SELECT memory_revision::text, next_entry_number::text
               FROM storyos.project_memory_heads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &values,
        )
        .await?;
    let (revision, next) = match head {
        Some(row) => (numeric(row.get(0))?, numeric(row.get(1))?),
        None => (0, 1),
    };
    let rows = client
        .query(
            "SELECT memory_entry_id::text, entry_number::text, version::text, entry_type, title,
                    description, text
               FROM (SELECT DISTINCT ON (memory_entry_id) *
                       FROM storyos.memory_entry_versions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                      ORDER BY memory_entry_id, memory_revision DESC) AS latest
              WHERE state = 'current'",
            &values,
        )
        .await?;
    let entries = rows
        .iter()
        .map(|row| {
            Ok(CurrentMemoryEntry {
                memory_entry_id: row.get(0),
                number: numeric(row.get(1))?,
                version: numeric(row.get(2))?,
                entry_type: MemoryEntryType::parse(row.get(3))
                    .ok_or_else(|| invalid("A Memory Entry type is invalid"))?,
                title: row.get(4),
                description: row.get(5),
                text: row.get(6),
            })
        })
        .collect::<Result<_, Box<dyn std::error::Error + Send + Sync>>>()?;
    Ok((revision, next, entries))
}

/// Locks the Memory head of the Project and gives its Memory revision. Each writer of a Memory
/// version takes the lock before it reads the Memory, so that the `write_memory` call of a Run and
/// the delete of the author move the Memory revision one at a time. A Project with no head has no
/// entry, no row to lock, and the revision 0.
pub(crate) async fn lock_memory_head(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT memory_revision::text FROM storyos.project_memory_heads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    row.map_or(Ok(0), |row| numeric(row.get(0)))
}

/// The last version of the entry `memory_entry_id` of the Project in each state, or `None` when
/// the Project has no entry with the identity.
pub(crate) async fn last_entry_version(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    memory_entry_id: &str,
) -> Result<Option<LastMemoryVersion>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            "SELECT memory_entry_id::text, entry_number::text, version::text, state, entry_type,
                    title, description
               FROM storyos.memory_entry_versions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND memory_entry_id = $3::text::uuid
              ORDER BY memory_revision DESC
              LIMIT 1",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &memory_entry_id,
            ],
        )
        .await?;
    row.map(|row| {
        Ok(LastMemoryVersion {
            memory_entry_id: row.get(0),
            number: numeric(row.get(1))?,
            version: numeric(row.get(2))?,
            state: MemoryEntryState::parse(row.get(3))
                .ok_or_else(|| invalid("A Memory Entry state is invalid"))?,
            entry_type: MemoryEntryType::parse(row.get(4))
                .ok_or_else(|| invalid("A Memory Entry type is invalid"))?,
            title: row.get(5),
            description: row.get(6),
        })
    })
    .transpose()
}

/// The time form of a Memory record: RFC 3339 in UTC with milliseconds, so that the order of
/// the texts is the order of the times.
pub(crate) const TIME_FORM: &str = r#"YYYY-MM-DD"T"HH24:MI:SS.MS"Z""#;

/// The time of the last change of the entry of the row `latest` (ADR 0094 section 7): the time of
/// its last version that is not a flag version.
pub(crate) const LAST_CHANGE: &str = "(SELECT changed.recorded_at
       FROM storyos.memory_entry_versions AS changed
      WHERE (changed.owner_user_id, changed.project_id, changed.memory_entry_id) =
            (latest.owner_user_id, latest.project_id, latest.memory_entry_id)
        AND changed.flag_reason IS NULL
      ORDER BY changed.memory_revision DESC
      LIMIT 1)";

/// The current version of the entry with the number `number` of the Project now, or `None` when no
/// current entry has the number. A flag version gives its reason and its source now.
pub(crate) async fn read_current_entry(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    number: i64,
) -> Result<Option<MemoryEntryRead>, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_opt(
            &format!(
                "SELECT memory_entry_id::text, version::text, entry_type, title, description,
                        origin, to_char({LAST_CHANGE} AT TIME ZONE 'UTC', $4), text,
                        flag_reason, flag_source::text
                   FROM (SELECT DISTINCT ON (memory_entry_id) *
                           FROM storyos.memory_entry_versions
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                            AND entry_number = $3::text::numeric
                          ORDER BY memory_entry_id, memory_revision DESC) AS latest
                  WHERE state = 'current'"
            ),
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &number.to_string(),
                &TIME_FORM,
            ],
        )
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let flag = match (
        row.get::<_, Option<String>>(8),
        row.get::<_, Option<String>>(9),
    ) {
        (Some(reason), Some(source)) => Some(MemoryReadFlag {
            reason,
            source: flag_source(client, scope, &source).await?,
        }),
        _ => None,
    };
    let read = (|| {
        Ok(MemoryEntryRead {
            memory_entry_id: row.get(0),
            version: numeric(row.get(1))?,
            entry_type: MemoryEntryType::parse(row.get(2))
                .ok_or_else(|| invalid("A Memory Entry type is invalid"))?,
            title: row.get(3),
            description: row.get(4),
            origin: row.get(5),
            changed_at: row.get(6),
            text: row.get(7),
            flag,
        })
    })();
    read.map(Some)
}

/// The source of a flag now: a live Chapter with its number in the Chapter list, a Project
/// Conversation of the Project, or a source that the Project does not have now.
async fn flag_source(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    source: &str,
) -> Result<MemoryFlagSource, Box<dyn std::error::Error + Send + Sync>> {
    let source = serde_json::from_str::<MemorySourceReference>(source).ok();
    Ok(match source {
        Some(MemorySourceReference::Chapter { chapter_id }) => {
            let chapters =
                crate::create_agent_run::context::chapter_list_entries(client, scope).await?;
            (chapters.into_iter().zip(1..))
                .find(|(chapter, _)| chapter.chapter_id == chapter_id)
                .map_or(MemoryFlagSource::Unavailable, |(chapter, number)| {
                    MemoryFlagSource::Chapter {
                        number,
                        title: chapter.title,
                    }
                })
        }
        Some(MemorySourceReference::Conversation {
            conversation_id, ..
        }) => {
            let found = client
                .query_opt(
                    "SELECT 1 FROM storyos.project_conversations
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND conversation_id::text = $3",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &conversation_id,
                    ],
                )
                .await?;
            match found {
                Some(_) => MemoryFlagSource::Conversation,
                None => MemoryFlagSource::Unavailable,
            }
        }
        None => MemoryFlagSource::Unavailable,
    })
}

/// For each current entry of the Project, in number order: its identity, its number, and the time
/// of its last read, or `None` when no Run read it. Only a read of a Run of the author counts
/// (ADR 0094 section 8), and the Memory Index is not a read. The start of a consolidation pass
/// uses it for the mark `unused`.
pub(crate) async fn last_memory_reads(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<Vec<(String, u64, Option<String>)>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT latest.memory_entry_id::text, latest.entry_number::text,
                    to_char(max(read.read_at) AT TIME ZONE 'UTC', $3)
               FROM (SELECT DISTINCT ON (memory_entry_id) *
                       FROM storyos.memory_entry_versions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                      ORDER BY memory_entry_id, memory_revision DESC) AS latest
               LEFT JOIN (storyos.memory_entry_reads AS read
                          JOIN storyos.agent_runs AS run
                            ON (run.owner_user_id, run.project_id, run.run_id) =
                               (read.owner_user_id, read.project_id, read.run_id)
                           AND run.run_kind = 'author')
                 ON (read.owner_user_id, read.project_id, read.memory_entry_id) =
                    (latest.owner_user_id, latest.project_id, latest.memory_entry_id)
              WHERE latest.state = 'current'
              GROUP BY latest.memory_entry_id, latest.entry_number
              ORDER BY latest.entry_number",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &TIME_FORM,
            ],
        )
        .await?;
    rows.iter()
        .map(|row| Ok((row.get(0), numeric(row.get(1))?, row.get(2))))
        .collect()
}

/// The origin, the cause, and the source references of one version.
pub(crate) struct VersionSource<'a> {
    pub origin: &'a str,
    pub cause: serde_json::Value,
    pub source_references: serde_json::Value,
}

/// Writes `version` as the next Memory revision of the Project after `revision`, and moves the
/// head. A new entry gets the identity `new_entry_id` and the next entry number after it. The flag
/// of a flag version goes into the columns `flag_reason` and `flag_source`.
pub(crate) async fn write_memory_version(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    (revision, version, new_entry_id): (u64, &MemoryVersionWrite, &str),
    source: &VersionSource<'_>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let flag_source = version
        .flag
        .as_ref()
        .map(|flag| serde_json::json!(flag.source).to_string());
    let next_number = match version.memory_entry_id {
        Some(_) => None,
        None => Some((version.number + 1).to_string()),
    };
    client
        .execute(
            "WITH head AS (
                 INSERT INTO storyos.project_memory_heads AS head
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric,
                         COALESCE($4::text::numeric, 1))
                 ON CONFLICT (owner_user_id, project_id) DO UPDATE
                    SET memory_revision = EXCLUDED.memory_revision,
                        next_entry_number = COALESCE($4::text::numeric, head.next_entry_number)
                 RETURNING memory_revision)
             INSERT INTO storyos.memory_entry_versions
                 (owner_user_id, project_id, memory_entry_id, entry_number, version,
                  memory_revision, state, entry_type, origin, title, description, text, cause,
                  source_references, flag_reason, flag_source)
             SELECT $1::text::uuid, $2::text::uuid, $5::text::uuid, $6::text::numeric,
                    $7::text::numeric, head.memory_revision, $8, $9, $10, $11, $12, $13,
                    $14::text::jsonb, $15::text::jsonb, $16, $17::text::jsonb
               FROM head",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &(revision + 1).to_string(),
                &next_number,
                &version.memory_entry_id.as_deref().unwrap_or(new_entry_id),
                &version.number.to_string(),
                &version.version.to_string(),
                &version.state.as_str(),
                &version.entry_type.as_str(),
                &source.origin,
                &version.title,
                &version.description,
                &version.text,
                &source.cause.to_string(),
                &source.source_references.to_string(),
                &version.flag.as_ref().map(|flag| flag.reason.as_str()),
                &flag_source,
            ],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "memory_entries_tests.rs"]
pub(crate) mod tests;
