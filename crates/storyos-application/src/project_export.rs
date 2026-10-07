use std::future::Future;

use storyos_core::{ExportProjectArchiveRefusal, ProjectArchiveBuildRefusal};

use crate::{
    AdmittedProjectCommand, CanonicalSnapshot, ProjectReadError, ProjectScope,
    RefusableCommandError,
};

pub const PROJECT_EXPORT_COMMAND_KIND: &str = "exportProjectArchive";
pub const PROJECT_EXPORT_ROUTE: &str = "/api/v1/projects/{project_id}/exports";
pub const PROJECT_EXPORT_REQUEST_SCHEMA: &str = "storyos.command.export-project-archive.request.v1";
pub const PROJECT_EXPORT_DIGEST_PROFILE: &str = "storyos.command.exportProjectArchive.jcs.v1";
pub const PROJECT_EXPORT_ARCHIVE_PROFILE: &str = "storyos.project-export.v1";
pub const PROJECT_EXPORT_ARCHIVE_PATH_PROFILE: &str =
    "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1";
pub const PROJECT_ARCHIVE_ZIP_MEDIA_TYPE: &str =
    "application/vnd.storyos.project-archive+zip; profile=\"storyos.project-export.v1\"";

/// The command-specific input of one exportProjectArchive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportProjectArchiveInput {
    /// The export identity of a first use. An exact retry returns the admitted identity.
    pub export_id: String,
}

/// The admitted Project Export Archive operation, its two profiles, and its pinned Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveExportOperation {
    pub export_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
    pub source_snapshot: CanonicalSnapshot,
}

pub type ExportProjectArchiveAdmission = AdmittedProjectCommand<ArchiveExportOperation>;

/// A refusal of exportProjectArchive before its Admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveExportRefusal {
    /// The Core classification of the Project lifecycle refuses the export.
    Lifecycle(ExportProjectArchiveRefusal),
    /// The exportable families cannot make a Project Export Archive.
    ArchiveBuild(ProjectArchiveBuildRefusal),
}

/// An archived Project and each archive build refusal refuse the export before its Admission.
pub type ExportProjectArchiveError = RefusableCommandError<ArchiveExportRefusal>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportOperationProgress {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
    pub source_snapshot: CanonicalSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportOperationPage {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
    pub source_snapshot: CanonicalSnapshot,
    pub immutable_root: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GetExportOperation {
    Missing,
    Archived,
    Expired,
    InProgress(Box<ExportOperationProgress>),
    Ready(Box<ExportOperationPage>),
    Failed(Box<ExportOperationProgress>),
    OutcomeUnknown(Box<ExportOperationProgress>),
}

/// Reads one admitted Project Export operation under already authenticated exact Scope.
pub trait ExportOperationReader: Sync {
    fn read_export_operation(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> impl Future<Output = Result<GetExportOperation, ProjectReadError>> + Send;

    fn read_verified_export_archive(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> impl Future<Output = Result<VerifiedExportArchive, ProjectReadError>> + Send;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedExportArchive {
    Missing,
    Archived,
    Expired,
    Unsettled,
    Ready(Vec<u8>),
    Refused(ProjectArchiveBuildRefusal),
}

pub async fn get_export_operation(
    reader: &impl ExportOperationReader,
    scope: &ProjectScope,
    export_id: &str,
) -> Result<GetExportOperation, ProjectReadError> {
    reader.read_export_operation(scope, export_id).await
}

pub async fn get_verified_export_archive(
    reader: &impl ExportOperationReader,
    scope: &ProjectScope,
    export_id: &str,
) -> Result<VerifiedExportArchive, ProjectReadError> {
    reader.read_verified_export_archive(scope, export_id).await
}

#[cfg(test)]
#[path = "project_export_tests.rs"]
mod tests;
