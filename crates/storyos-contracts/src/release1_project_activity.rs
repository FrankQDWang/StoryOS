//! Implemented public Project Activity kinds and compatible Event schemas.

macro_rules! activity_kinds {
    ($( $variant:ident => ($kind:literal, $schema:literal $(, $later_schema:literal)?), )*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        /// Names one implemented public Project Activity Event kind.
        pub enum ProjectActivityKind {
            $( $variant, )*
        }

        impl ProjectActivityKind {
            /// Lists every kind that the current release can persist and replay.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// Resolves a persisted kind, or refuses an unknown kind.
            pub fn from_persisted(kind: &str) -> Option<Self> {
                match kind {
                    $( $kind => Some(Self::$variant), )*
                    _ => None,
                }
            }

            /// Returns the persisted and public Event kind name.
            pub fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $kind, )* }
            }

            /// Returns the public Event schemas accepted for this kind.
            pub fn event_schemas(self) -> &'static [&'static str] {
                match self { $( Self::$variant => &[ $schema $(, $later_schema)? ], )* }
            }

            /// Returns the default public Event schema for this kind.
            pub fn event_schema(self) -> &'static str {
                self.event_schemas()[0]
            }

            /// Selects the creation Event schema for a receipt with optional order.
            pub fn event_schema_for_create_receipt(self, receipt_order: Option<&str>) -> &'static str {
                if receipt_order.is_some_and(|order| !order.is_empty()) {
                    match self {
                        $( Self::$variant => { $( return $later_schema; )? }, )*
                    }
                }
                self.event_schema()
            }
        }
    };
}

activity_kinds! {
    AuthoritativeAuthorEditApplied => ("authoritative_author_edit_applied", "storyos.event.authoritative-author-edit-applied.v1"),
    WriterTakeoverApplied => ("writer_takeover_applied", "storyos.event.writer-takeover-applied.v1"),
    WriterTakeoverCompareFailed => ("writer_takeover_compare_failed", "storyos.event.writer-takeover-compare-failed.v1"),
    ProjectCreated => ("project_created", "storyos.event.project-created.v1"),
    ProjectUpdated => ("project_updated", "storyos.event.project-updated.v1"),
    ProjectArchivalChanged => ("project_archival_changed", "storyos.event.project-archival-changed.v1"),
    ProjectAssistanceUpdated => ("project_assistance_updated", "storyos.event.project-assistance-updated.v1"),
    VolumeCreated => ("volume_created", "storyos.event.volume-created.v1", "storyos.event.volume-created.v2"),
    VolumeUpdated => ("volume_updated", "storyos.event.volume-updated.v1"),
    ChapterCreated => ("chapter_created", "storyos.event.chapter-created.v1", "storyos.event.chapter-created.v2"),
    ChapterUpdated => ("chapter_updated", "storyos.event.chapter-updated.v1"),
    CurrentChapterSet => ("current_chapter_set", "storyos.event.current-chapter-set.v1"),
    ChapterDeleted => ("chapter_deleted", "storyos.event.chapter-deleted.v1"),
    VolumeDeleted => ("volume_deleted", "storyos.event.volume-deleted.v1"),
    HumanReadableManuscriptExportSettled => ("human_readable_manuscript_export_settled", "storyos.event.human-readable-manuscript-export-settled.v1"),
    ProjectExportSettled => ("project_export_settled", "storyos.event.project-export-settled.v1"),
    AgentRunCreated => ("agent_run_created", "storyos.event.agent-run-created.v1"),
    AgentRunPaused => ("agent_run_paused", "storyos.event.agent-run-paused.v1"),
    AgentRunCancelled => ("agent_run_cancelled", "storyos.event.agent-run-cancelled.v1"),
}
