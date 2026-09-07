use storyos_core::ReadableExportVolume;

use crate::ProjectScope;

/// Completeness stored in one Application-owned Pinned Export Source.
///
/// Both export journeys use this type. They differ only by the stored facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinnedExportSourceFacts {
    HumanReadableManuscript { volumes: Vec<ReadableExportVolume> },
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
    }
}
