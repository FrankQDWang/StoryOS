//! The query `listMemoryEntries` of the settings page (ADR 0094 section 9).

use storyos_application::{MemoryEntrySourceItem, list_memory_entries as list_entries};
use storyos_core::{MemoryEntryOrigin, MemoryEntryType};

use super::*;

pub(super) async fn list_memory_entries(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::ListMemoryEntriesResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state).await?;
    let Some(entries) = list_entries(&reader, &scope)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::ListMemoryEntriesResponse {
        schema_id: contracts::LIST_MEMORY_ENTRIES_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        entries: entries
            .into_iter()
            .map(|entry| contracts::MemoryEntryInspect {
                memory_entry_id: entry.memory_entry_id,
                entry_number: entry.number.to_string(),
                version: entry.version.to_string(),
                entry_type: entry_type(entry.entry_type),
                title: entry.title,
                description: entry.description,
                text: entry.text,
                origin: origin(entry.origin),
                source: entry.source.map(source),
                changed_at: entry.changed_at,
                flag: entry.flag.map(|flag| contracts::MemoryEntryFlagInspect {
                    reason: flag.reason,
                    source: source(flag.source),
                    flagged_at: flag.flagged_at,
                }),
                intent: entry
                    .intent
                    .map(|intent| contracts::MemoryEntryIntentInspect {
                        text: intent.text,
                        questions: intent.questions,
                        chapter: intent.chapter.map(source),
                        recorded_at: intent.recorded_at,
                    }),
            })
            .collect(),
    }))
}

fn entry_type(entry_type: MemoryEntryType) -> contracts::MemoryEntryTypeInspect {
    match entry_type {
        MemoryEntryType::WritingPreference => contracts::MemoryEntryTypeInspect::WritingPreference,
        MemoryEntryType::Foreshadowing => contracts::MemoryEntryTypeInspect::Foreshadowing,
        MemoryEntryType::Character => contracts::MemoryEntryTypeInspect::Character,
        MemoryEntryType::Relationship => contracts::MemoryEntryTypeInspect::Relationship,
        MemoryEntryType::MajorEvent => contracts::MemoryEntryTypeInspect::MajorEvent,
        MemoryEntryType::Other => contracts::MemoryEntryTypeInspect::Other,
    }
}

fn origin(origin: MemoryEntryOrigin) -> contracts::MemoryEntryOriginInspect {
    match origin {
        MemoryEntryOrigin::AuthorRequest => contracts::MemoryEntryOriginInspect::AuthorRequest,
        MemoryEntryOrigin::Conversation => contracts::MemoryEntryOriginInspect::Conversation,
        MemoryEntryOrigin::Chapter => contracts::MemoryEntryOriginInspect::Chapter,
    }
}

fn availability(available: bool) -> contracts::MemorySourceAvailability {
    match available {
        true => contracts::MemorySourceAvailability::Available,
        false => contracts::MemorySourceAvailability::Unavailable,
    }
}

fn source(source: MemoryEntrySourceItem) -> contracts::MemoryEntrySourceInspect {
    match source {
        MemoryEntrySourceItem::Conversation {
            conversation_id,
            available,
        } => contracts::MemoryEntrySourceInspect::Conversation {
            conversation_id,
            availability: availability(available),
        },
        MemoryEntrySourceItem::Chapter {
            chapter_id,
            title,
            available,
        } => contracts::MemoryEntrySourceInspect::Chapter {
            chapter_id,
            title,
            availability: availability(available),
        },
    }
}
