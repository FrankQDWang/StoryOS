//! The catalog Events: the Project Activity Events that the code can persist and replay.

use serde_json::{Value, json};

use super::Owner;
use crate::ProjectActivityKind;
use crate::release1::ACTIVITY_PROFILE;

/// One catalog Event for each Event schema of each Project Activity kind that the code can persist.
pub(super) fn catalog_events() -> impl Iterator<Item = Value> {
    ProjectActivityKind::ALL.iter().flat_map(|kind| {
        kind.event_schemas().iter().map(|schema_id| {
            json!({
                "schema_id": schema_id, "event_kind": kind.as_str(),
                "semantic_owner": event_owner(*kind), "wire_profile": ACTIVITY_PROFILE,
                "description": event_description(schema_id),
                "positive_fixture": format!("storyos.golden.{schema_id}.positive.v1"),
                "negative_fixture": format!("storyos.golden.{schema_id}.negative.v1"),
            })
        })
    })
}

fn event_owner(kind: ProjectActivityKind) -> Owner {
    match kind {
        ProjectActivityKind::AuthoritativeAuthorEditApplied
        | ProjectActivityKind::CurrentChapterSet => Owner::Core,
        ProjectActivityKind::WriterTakeoverApplied
        | ProjectActivityKind::WriterTakeoverCompareFailed => Owner::Editor,
        ProjectActivityKind::ProjectCreated
        | ProjectActivityKind::ProjectUpdated
        | ProjectActivityKind::ProjectAssistanceUpdated
        | ProjectActivityKind::VolumeCreated
        | ProjectActivityKind::VolumeUpdated
        | ProjectActivityKind::VolumeDeleted
        | ProjectActivityKind::ChapterCreated
        | ProjectActivityKind::ChapterUpdated
        | ProjectActivityKind::ChapterDeleted
        | ProjectActivityKind::HumanReadableManuscriptExportSettled => Owner::Release,
        ProjectActivityKind::ProjectArchivalChanged => Owner::Retention,
        ProjectActivityKind::ProjectExportSettled => Owner::Storage,
        ProjectActivityKind::AgentRunCreated
        | ProjectActivityKind::AgentRunSteeringRetained
        | ProjectActivityKind::AgentRunPaused
        | ProjectActivityKind::AgentRunCancelled => Owner::Context,
    }
}

fn event_description(schema_id: &str) -> &'static str {
    match schema_id {
        "storyos.event.volume-created.v1" | "storyos.event.chapter-created.v1" => {
            "Historical create Activity. The order field is the stored storage key."
        }
        "storyos.event.volume-created.v2" | "storyos.event.chapter-created.v2" => {
            "New create Activity. The order field is Canonical Sibling Order."
        }
        "storyos.event.project-archival-changed.v1" => {
            "Historical archive Activity. No command writes it since ADR 0078."
        }
        "storyos.event.agent-run-steering-retained.v1" => {
            "Ordered author input retained for the next safe decision."
        }
        "storyos.event.human-readable-manuscript-export-settled.v1" => {
            "Human-readable manuscript export settled from a deterministic volume/chapter source order; unavailable content is represented by the export profile."
        }
        _ => "",
    }
}
