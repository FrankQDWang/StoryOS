//! The reads of Agent Memory for a Run (ADR 0094 sections 3 and 4): the pin of the Memory
//! revision and the current Memory Entries at a revision. No diagnostic records a title, a
//! description, or a text of an entry (ADR 0047).

use storyos_application::ProjectScope;
use storyos_core::{MemoryEntryType, MemoryIndexEntry};

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
/// with the highest Memory revision that is not above it, when its state is `current`.
pub(crate) async fn read_memory_index(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    memory_revision: u64,
) -> Result<Vec<MemoryIndexEntry>, Box<dyn std::error::Error + Send + Sync>> {
    let rows = client
        .query(
            "SELECT entry_number::text, entry_type, title, description
               FROM (SELECT DISTINCT ON (memory_entry_id)
                            entry_number, state, entry_type, title, description
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
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "memory_entries_tests.rs"]
pub(crate) mod tests;
