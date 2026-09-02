/// Event kinds that both current Activity table families and the Release 1
/// route catalog already admit. Stage 3 kinds are not in this set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectActivityKind {
    AuthoritativeAuthorEditApplied,
    WriterTakeoverApplied,
    WriterTakeoverCompareFailed,
    ProjectCreated,
    ProjectUpdated,
    ProjectArchivalChanged,
    VolumeCreated,
    VolumeUpdated,
    ChapterCreated,
    ChapterUpdated,
    CurrentChapterSet,
    ChapterDeleted,
    VolumeDeleted,
    HumanReadableManuscriptExportSettled,
    ProjectExportSettled,
}

impl ProjectActivityKind {
    pub fn from_persisted(kind: &str) -> Option<Self> {
        Some(match kind {
            "authoritative_author_edit_applied" => Self::AuthoritativeAuthorEditApplied,
            "writer_takeover_applied" => Self::WriterTakeoverApplied,
            "writer_takeover_compare_failed" => Self::WriterTakeoverCompareFailed,
            "project_created" => Self::ProjectCreated,
            "project_updated" => Self::ProjectUpdated,
            "project_archival_changed" => Self::ProjectArchivalChanged,
            "volume_created" => Self::VolumeCreated,
            "volume_updated" => Self::VolumeUpdated,
            "chapter_created" => Self::ChapterCreated,
            "chapter_updated" => Self::ChapterUpdated,
            "current_chapter_set" => Self::CurrentChapterSet,
            "chapter_deleted" => Self::ChapterDeleted,
            "volume_deleted" => Self::VolumeDeleted,
            "human_readable_manuscript_export_settled" => {
                Self::HumanReadableManuscriptExportSettled
            }
            "project_export_settled" => Self::ProjectExportSettled,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthoritativeAuthorEditApplied => "authoritative_author_edit_applied",
            Self::WriterTakeoverApplied => "writer_takeover_applied",
            Self::WriterTakeoverCompareFailed => "writer_takeover_compare_failed",
            Self::ProjectCreated => "project_created",
            Self::ProjectUpdated => "project_updated",
            Self::ProjectArchivalChanged => "project_archival_changed",
            Self::VolumeCreated => "volume_created",
            Self::VolumeUpdated => "volume_updated",
            Self::ChapterCreated => "chapter_created",
            Self::ChapterUpdated => "chapter_updated",
            Self::CurrentChapterSet => "current_chapter_set",
            Self::ChapterDeleted => "chapter_deleted",
            Self::VolumeDeleted => "volume_deleted",
            Self::HumanReadableManuscriptExportSettled => {
                "human_readable_manuscript_export_settled"
            }
            Self::ProjectExportSettled => "project_export_settled",
        }
    }

    pub fn event_schema(self) -> &'static str {
        match self {
            Self::AuthoritativeAuthorEditApplied => {
                "storyos.event.authoritative-author-edit-applied.v1"
            }
            Self::WriterTakeoverApplied => "storyos.event.writer-takeover-applied.v1",
            Self::WriterTakeoverCompareFailed => "storyos.event.writer-takeover-compare-failed.v1",
            Self::ProjectCreated => "storyos.event.project-created.v1",
            Self::ProjectUpdated => "storyos.event.project-updated.v1",
            Self::ProjectArchivalChanged => "storyos.event.project-archival-changed.v1",
            Self::VolumeCreated => "storyos.event.volume-created.v1",
            Self::VolumeUpdated => "storyos.event.volume-updated.v1",
            Self::ChapterCreated => "storyos.event.chapter-created.v1",
            Self::ChapterUpdated => "storyos.event.chapter-updated.v1",
            Self::CurrentChapterSet => "storyos.event.current-chapter-set.v1",
            Self::ChapterDeleted => "storyos.event.chapter-deleted.v1",
            Self::VolumeDeleted => "storyos.event.volume-deleted.v1",
            Self::HumanReadableManuscriptExportSettled => {
                "storyos.event.human-readable-manuscript-export-settled.v1"
            }
            Self::ProjectExportSettled => "storyos.event.project-export-settled.v1",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityAggregateRef {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectActivityEvent {
    pub event_id: String,
    pub kind: ProjectActivityKind,
    pub project_sequence: u64,
    pub stream_sequence: u64,
    pub command_id: String,
    pub correlation_id: String,
    pub receipt_id: String,
    pub occurred_at: String,
    pub aggregate: ActivityAggregateRef,
    pub payload_json: String,
}

#[cfg(test)]
#[path = "project_activity_tests.rs"]
mod tests;
