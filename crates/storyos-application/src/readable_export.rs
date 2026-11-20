use std::future::Future;

use storyos_core::{ReadableExportChapter, ReadableExportVolume};

use crate::{
    AdmittedProjectCommand, CanonicalSnapshot, CanonicalTreeFacts, ManuscriptSearchChapterFact,
    ProjectReadError, ProjectScope, RefusableCommandError,
};

/// The command-specific input of one exportHumanReadableManuscript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportHumanReadableManuscriptInput {
    /// The export identity of a first use. An exact retry returns the admitted identity.
    pub export_id: String,
}

/// The admitted human-readable export operation and its pinned Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadableExportOperation {
    pub export_id: String,
    pub source_snapshot: CanonicalSnapshot,
}

pub type ExportHumanReadableManuscriptAdmission = AdmittedProjectCommand<ReadableExportOperation>;

/// A missing Project refuses the export before its Admission.
pub type ExportHumanReadableManuscriptError =
    RefusableCommandError<storyos_core::ExportHumanReadableManuscriptRefusal>;

/// Join live tree titles with canonical Block facts. A live Chapter without a
/// loadable payload is an explicit gap, never invented prose.
pub fn readable_volumes_from_canonical_facts(
    tree: &CanonicalTreeFacts,
    chapters: &[ManuscriptSearchChapterFact],
) -> Vec<ReadableExportVolume> {
    tree.volumes
        .iter()
        .map(|volume| ReadableExportVolume {
            title: volume.title.clone(),
            chapters: volume
                .chapters
                .iter()
                .map(|chapter| ReadableExportChapter {
                    title: chapter.title.clone(),
                    body: chapters.iter().find_map(|fact| {
                        (fact.chapter_id == chapter.chapter_id).then(|| {
                            fact.blocks
                                .iter()
                                .map(|block| block.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                    }),
                })
                .collect(),
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanReadableManuscriptExportProgress {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub export_profile: String,
    pub source_snapshot: CanonicalSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanReadableManuscriptExportPage {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub export_profile: String,
    pub content_sha256: String,
    pub manuscript_utf8: String,
    pub source_snapshot: CanonicalSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GetHumanReadableManuscriptExport {
    Missing,
    Expired,
    InProgress(Box<HumanReadableManuscriptExportProgress>),
    Ready(Box<HumanReadableManuscriptExportPage>),
    Failed(Box<HumanReadableManuscriptExportProgress>),
    OutcomeUnknown(Box<HumanReadableManuscriptExportProgress>),
}

/// Reads one human-readable export under already authenticated exact Scope.
pub trait HumanReadableManuscriptExportReader: Sync {
    fn read_human_readable_manuscript_export(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> impl Future<Output = Result<GetHumanReadableManuscriptExport, ProjectReadError>> + Send;
}

pub async fn get_human_readable_manuscript_export(
    reader: &impl HumanReadableManuscriptExportReader,
    scope: &ProjectScope,
    export_id: &str,
) -> Result<GetHumanReadableManuscriptExport, ProjectReadError> {
    reader
        .read_human_readable_manuscript_export(scope, export_id)
        .await
}

#[cfg(test)]
#[path = "readable_export_tests.rs"]
mod tests;
