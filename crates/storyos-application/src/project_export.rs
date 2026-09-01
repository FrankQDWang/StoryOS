use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, CanonicalSnapshot, EditorClientBinding,
    ProjectCommandChallengeBinding, ProjectReadError, ProjectScope,
};

pub const PROJECT_EXPORT_COMMAND_KIND: &str = "exportProjectArchive";
pub const PROJECT_EXPORT_ROUTE: &str = "/api/v1/projects/{project_id}/exports";
pub const PROJECT_EXPORT_REQUEST_SCHEMA: &str = "storyos.command.export-project-archive.request.v1";
pub const PROJECT_EXPORT_DIGEST_PROFILE: &str = "storyos.command.exportProjectArchive.jcs.v1";
pub const PROJECT_EXPORT_ARCHIVE_PROFILE: &str = "storyos.project-export.v1";
pub const PROJECT_EXPORT_ARCHIVE_PATH_PROFILE: &str =
    "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportProjectArchiveCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub export_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
    pub source_snapshot: CanonicalSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportProjectArchiveSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub export_id: String,
    pub effect: ExportProjectArchiveSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportProjectArchiveSettlementEffect {
    Admitted {
        archive_profile: String,
        archive_path_profile: String,
        source_snapshot: CanonicalSnapshot,
    },
    Refused {
        reason: ExportProjectArchiveRefusal,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportProjectArchiveRefusal {
    MissingProject,
    ArchivedProject,
}

#[derive(Debug)]
pub enum ExportProjectArchiveError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ExportProjectArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => {
                formatter.write_str("The Project Export Archive binding conflicts")
            }
            Self::InvalidChallenge => {
                formatter.write_str("The Project Export Archive challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => {
                formatter.write_str("The Project Export Archive store is unavailable")
            }
        }
    }
}

impl std::error::Error for ExportProjectArchiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Project Export Archive operation and its atomic settlement.
pub trait ExportProjectArchiveStore: Sync {
    fn export_project_archive(
        &self,
        command: &ExportProjectArchiveCommand,
    ) -> impl Future<Output = Result<ExportProjectArchiveSettlement, ExportProjectArchiveError>> + Send;
}

pub async fn request_export_project_archive(
    store: &impl ExportProjectArchiveStore,
    command: &ExportProjectArchiveCommand,
) -> Result<ExportProjectArchiveSettlement, ExportProjectArchiveError> {
    let challenge = &command.challenge_binding;
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&command.canonical_command_bytes)
            .iter()
            .fold(String::with_capacity(64), |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            });
        format!("sha256:{PROJECT_EXPORT_DIGEST_PROFILE}:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != PROJECT_EXPORT_COMMAND_KIND
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != PROJECT_EXPORT_ROUTE
        || challenge.command_schema != PROJECT_EXPORT_REQUEST_SCHEMA
        || command.archive_profile != PROJECT_EXPORT_ARCHIVE_PROFILE
        || command.archive_path_profile != PROJECT_EXPORT_ARCHIVE_PATH_PROFILE
    {
        return Err(ExportProjectArchiveError::BindingConflict);
    }
    store.export_project_archive(command).await
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportOperationPage {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
    pub source_snapshot: CanonicalSnapshot,
    pub immutable_root: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GetExportOperation {
    Missing,
    Archived,
    InProgress(Box<ExportOperationPage>),
}

/// Reads one admitted Project Export operation under already authenticated exact Scope.
pub trait ExportOperationReader: Sync {
    fn read_export_operation(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> impl Future<Output = Result<GetExportOperation, ProjectReadError>> + Send;
}

pub async fn get_export_operation(
    reader: &impl ExportOperationReader,
    scope: &ProjectScope,
    export_id: &str,
) -> Result<GetExportOperation, ProjectReadError> {
    reader.read_export_operation(scope, export_id).await
}

#[cfg(test)]
#[path = "project_export_tests.rs"]
mod tests;
