use storyos_core::ReadableExportVolume;

use crate::ProjectScope;

/// One exportable Archive family frozen at admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinnedArchiveFamily {
    pub table: String,
    pub path: String,
    pub rows_json: String,
}

/// Completeness stored in one Application-owned Pinned Export Source.
///
/// Both export journeys use this type. They differ only by the stored facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinnedExportSourceFacts {
    HumanReadableManuscript { volumes: Vec<ReadableExportVolume> },
    ProjectExportArchive { families: Vec<PinnedArchiveFamily> },
}

/// Frozen exportable facts bound to one admitted export and its Snapshot locator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinnedExportSource {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub source_snapshot_id: String,
    pub facts: PinnedExportSourceFacts,
}

pub fn render_readable_manuscript_from_pinned_source(source: &PinnedExportSource) -> String {
    match &source.facts {
        PinnedExportSourceFacts::HumanReadableManuscript { volumes } => {
            storyos_core::render_readable_manuscript(volumes)
        }
        PinnedExportSourceFacts::ProjectExportArchive { .. } => {
            unreachable!("Archive facts cannot render a human-readable manuscript")
        }
    }
}
