use std::future::Future;

use storyos_core::{ReadableExportChapter, ReadableExportVolume, render_readable_manuscript};

use crate::{
    AuthorCommandAdmissionIds, CanonicalSnapshot, CanonicalTreeFacts, EditorClientBinding,
    ManuscriptSearchChapterFact, ProjectCommandChallengeBinding, ProjectReadError, ProjectScope,
};

pub const HUMAN_READABLE_EXPORT_COMMAND_KIND: &str = "exportHumanReadableManuscript";
pub const HUMAN_READABLE_EXPORT_ROUTE: &str = "/api/v1/projects/{project_id}/manuscript/exports";
pub const HUMAN_READABLE_EXPORT_REQUEST_SCHEMA: &str =
    "storyos.command.export-human-readable-manuscript.request.v1";
pub const HUMAN_READABLE_EXPORT_DIGEST_PROFILE: &str =
    "storyos.command.exportHumanReadableManuscript.jcs.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportHumanReadableManuscriptCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub export_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportHumanReadableManuscriptSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub export_id: String,
    pub effect: ExportHumanReadableManuscriptSettlementEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportHumanReadableManuscriptSettlementEffect {
    Admitted {
        source_snapshot: Box<CanonicalSnapshot>,
    },
    Refused {
        reason: storyos_core::ExportHumanReadableManuscriptRefusal,
    },
}

#[derive(Debug)]
pub enum ExportHumanReadableManuscriptError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    ArchivedProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ExportHumanReadableManuscriptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => {
                formatter.write_str("The human-readable export binding conflicts")
            }
            Self::InvalidChallenge => {
                formatter.write_str("The human-readable export challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::ArchivedProject => formatter.write_str("The Project is archived"),
            Self::Unavailable(_) => {
                formatter.write_str("The human-readable export store is unavailable")
            }
        }
    }
}

impl std::error::Error for ExportHumanReadableManuscriptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict
            | Self::InvalidChallenge
            | Self::MissingProject
            | Self::ArchivedProject => None,
        }
    }
}

/// Owns one admitted human-readable export and its atomic Core settlement.
pub trait ExportHumanReadableManuscriptStore: Sync {
    fn export_human_readable_manuscript(
        &self,
        command: &ExportHumanReadableManuscriptCommand,
    ) -> impl Future<
        Output = Result<
            ExportHumanReadableManuscriptSettlement,
            ExportHumanReadableManuscriptError,
        >,
    > + Send;
}

pub async fn request_human_readable_manuscript_export(
    store: &impl ExportHumanReadableManuscriptStore,
    command: &ExportHumanReadableManuscriptCommand,
) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
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
        format!("sha256:{HUMAN_READABLE_EXPORT_DIGEST_PROFILE}:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != HUMAN_READABLE_EXPORT_COMMAND_KIND
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != HUMAN_READABLE_EXPORT_ROUTE
        || challenge.command_schema != HUMAN_READABLE_EXPORT_REQUEST_SCHEMA
    {
        return Err(ExportHumanReadableManuscriptError::BindingConflict);
    }
    store.export_human_readable_manuscript(command).await
}

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

pub fn render_readable_manuscript_from_facts(
    tree: &CanonicalTreeFacts,
    chapters: &[ManuscriptSearchChapterFact],
) -> String {
    render_readable_manuscript(&readable_volumes_from_canonical_facts(tree, chapters))
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
    Archived,
    Expired,
    InProgress(Box<HumanReadableManuscriptExportProgress>),
    Ready(Box<HumanReadableManuscriptExportPage>),
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
