//! The current entries of a consolidation pass and the versions that its settlement writes (ADR
//! 0094 sections 6, 7, and 8). The entries have their sources and the mark `unused`. Each admitted
//! operation writes its version. A merge also writes the deleted version of each merged entry.
//! A conflict flag writes a flag version.

use serde_json::{Value, json};
use storyos_application::{CompleteAgentRunError, ProjectScope};
use storyos_core::{
    AdmittedOperation, CurrentMemoryEntry, MemoryEntryOrigin, MemoryEntryType,
    MemorySourceReference, PassConflict, PassMemoryEntry, PassObservation,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::agent_run_tool_authority::damaged;
use crate::memory_entries::{LAST_CHANGE, TIME_FORM, VersionSource, write_memory_version};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The numbers of the entries of `entries` that Core gives the mark `unused`, from the time of the
/// last read of each entry by a Run of the author, the time of its last change, and the time now.
pub(crate) async fn unused_entries(
    client: &Client,
    scope: &ProjectScope,
    entries: &[(PassMemoryEntry, String)],
) -> Result<Vec<u64>, BoxError> {
    let reads = crate::memory_entries::last_memory_reads(client, scope).await?;
    let now: String = client
        .query_one(
            "SELECT to_char(statement_timestamp() AT TIME ZONE 'UTC', $1)",
            &[&TIME_FORM],
        )
        .await?
        .get(/*idx*/ 0);
    let unused = entries.iter().filter(|(pass_entry, changed_at)| {
        let number = pass_entry.entry.number;
        let read = reads
            .iter()
            .find(|(_, read_number, _)| *read_number == number);
        let last_read = read.and_then(|(_, _, time)| time.as_deref());
        storyos_core::memory_entry_unused(pass_entry.origin, last_read, changed_at, &now)
    });
    Ok(unused
        .map(|(pass_entry, _)| pass_entry.entry.number)
        .collect())
}

/// One item of the input of a pass: an observation of the request, or a conflict that the Host
/// makes a flag (ADR 0094 section 7).
#[derive(Clone)]
pub(crate) enum PassItem {
    Observation(PassObservation),
    Conflict(PassConflict),
}

/// The item of a row with the columns capture, position, type, title, text, Conversation,
/// Chapter, entry number, and reason. The source is the source of the capture.
pub(crate) fn pass_item(row: &tokio_postgres::Row) -> Result<([String; 2], PassItem), BoxError> {
    let source = match (row.get(5), row.get(6)) {
        (Some(conversation_id), _) => MemorySourceReference::Conversation {
            conversation_id,
            run_id: None,
            tool_call_id: None,
        },
        (None, Some(chapter_id)) => MemorySourceReference::Chapter { chapter_id },
        (None, None) => return Err("A capture has no source".into()),
    };
    let item = match row.get::<_, Option<&str>>(7) {
        Some(entry) => PassItem::Conflict(PassConflict {
            entry: entry.parse()?,
            reason: row.get(8),
            source,
        }),
        None => PassItem::Observation(PassObservation {
            entry_type: MemoryEntryType::parse(row.get(2))
                .ok_or("A Memory Observation type is invalid")?,
            title: row.get(3),
            text: row.get(4),
            source,
        }),
    };
    Ok(([row.get(0), row.get(1)], item))
}

/// The keys and the observations of the request of `items`, in order, and the conflicts with
/// their keys.
#[expect(clippy::type_complexity, reason = "three parallel lists of one input")]
pub(crate) fn split_items(
    items: Vec<([String; 2], PassItem)>,
) -> (
    Vec<[String; 2]>,
    Vec<PassObservation>,
    Vec<([String; 2], PassConflict)>,
) {
    let (mut keys, mut observations, mut conflicts) = (Vec::new(), Vec::new(), Vec::new());
    for (key, item) in items {
        match item {
            PassItem::Observation(observation) => {
                keys.push(key);
                observations.push(observation);
            }
            PassItem::Conflict(conflict) => conflicts.push((key, conflict)),
        }
    }
    (keys, observations, conflicts)
}

/// One current entry of the Project now, with the time of its last change and its source
/// references.
pub(crate) struct StoredEntry {
    pub(crate) entry: PassMemoryEntry,
    pub(crate) changed_at: String,
    pub(crate) sources: Vec<Value>,
}

/// The last version of each current entry of the Project now, in the order of the numbers.
pub(crate) async fn pass_entries(
    client: &Client,
    scope: &ProjectScope,
) -> Result<Vec<StoredEntry>, BoxError> {
    let rows = client
        .query(
            &format!(
                "SELECT memory_entry_id::text, entry_number::text, version::text, entry_type,
                        title, description, text, origin, to_char({LAST_CHANGE} AT TIME ZONE 'UTC',
                        '{TIME_FORM}'), source_references::text, flag_reason IS NOT NULL
                   FROM (SELECT DISTINCT ON (memory_entry_id) *
                           FROM storyos.memory_entry_versions
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                          ORDER BY memory_entry_id, memory_revision DESC) AS latest
                  WHERE state = 'current'
                  ORDER BY entry_number"
            ),
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    let invalid = || Box::new(std::io::Error::other("A Memory Entry row is invalid"));
    rows.iter()
        .map(|row| {
            let number = |index| row.get::<_, &str>(index).parse().map_err(|_| invalid());
            Ok(StoredEntry {
                entry: PassMemoryEntry {
                    entry: CurrentMemoryEntry {
                        memory_entry_id: row.get(0),
                        number: number(1)?,
                        version: number(2)?,
                        entry_type: MemoryEntryType::parse(row.get(3)).ok_or_else(invalid)?,
                        title: row.get(4),
                        description: row.get(5),
                        text: row.get(6),
                        flagged: row.get(10),
                    },
                    origin: MemoryEntryOrigin::parse(row.get(7)).ok_or_else(invalid)?,
                },
                changed_at: row.get(8),
                sources: serde_json::from_str(row.get(9))?,
            })
        })
        .collect()
}

/// Writes the versions of the admitted operation `admitted` from the Memory revision `written`,
/// and returns the Memory revision after them. A flag version keeps the sources of its entry.
pub(crate) async fn write_admitted(
    client: &Client,
    scope: &ProjectScope,
    mut written: u64,
    admitted: &AdmittedOperation,
    stored: &[StoredEntry],
    cause: &Value,
) -> Result<u64, CompleteAgentRunError> {
    let unavailable = CompleteAgentRunError::Unavailable;
    let stored_of = |number: u64| stored.iter().find(|item| item.entry.entry.number == number);
    // The result holds the sources of its entry, of each merged entry, and of the
    // observations. A merged entry keeps its own sources in its deleted version.
    let numbers = [&admitted.version].into_iter().chain(&admitted.merged);
    let all = numbers
        .filter_map(|version| stored_of(version.number))
        .flat_map(|item| item.sources.iter().cloned())
        .chain(admitted.sources.iter().map(|source| json!(source)));
    let mut unique: Vec<Value> = Vec::new();
    for source in all {
        if !unique.contains(&source) {
            unique.push(source);
        }
    }
    // A flag version has the cause `conflict_flag` (ADR 0094 section 7).
    let mut version_cause = cause.clone();
    if admitted.version.flag.is_some() {
        version_cause["kind"] = json!("conflict_flag");
    }
    let source = VersionSource {
        origin: admitted.origin.as_str(),
        cause: version_cause,
        source_references: Value::from(unique),
    };
    let new_entry_id = Uuid::now_v7().to_string();
    let version = (written, &admitted.version, new_entry_id.as_str());
    write_memory_version(client, scope, version, &source)
        .await
        .map_err(unavailable)?;
    written += 1;
    for merged in &admitted.merged {
        let item = stored_of(merged.number).ok_or_else(|| damaged("A merged entry is missing"))?;
        let source = VersionSource {
            origin: item.entry.origin.as_str(),
            cause: cause.clone(),
            source_references: Value::from(item.sources.clone()),
        };
        write_memory_version(client, scope, (written, merged, ""), &source)
            .await
            .map_err(unavailable)?;
        written += 1;
    }
    Ok(written)
}
