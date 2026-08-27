//! Project query use cases and inward-facing ports.
//!
//! Durable identities cannot be exchanged at the Application boundary:
//!
//! ```compile_fail
//! use storyos_application::{ProjectId, ProjectScope, UserId};
//!
//! let _scope = ProjectScope::new(
//!     ProjectId::new("018f0000-0000-7001-8000-000000000002"),
//!     UserId::new("018f0000-0000-7001-8000-000000000001"),
//! );
//! ```
//!
//! ```compile_fail
//! use storyos_application::{Chapter, ChapterId, RevisionId};
//!
//! let _chapter = Chapter {
//!     chapter_id: RevisionId::new("018f0000-0000-7001-8000-000000000005"),
//!     title: "Chapter".to_owned(),
//!     revision_id: ChapterId::new("018f0000-0000-7001-8000-000000000003"),
//!     body: "Body".to_owned(),
//! };
//! ```

use std::future::Future;

mod archive_project;
mod author_command_outcome_unknown;
mod author_edit;
mod author_edit_outcome;
mod chapter_query;
mod create_chapter;
mod create_project;
mod create_project_challenge;
mod create_volume;
mod editor_session;
mod list_projects;
mod manuscript_tree;
mod snapshot;
mod takeover;
mod update_project;
mod update_volume;

#[cfg(test)]
#[path = "author_command_outcome_unknown_tests.rs"]
mod author_command_outcome_unknown_tests;

pub use author_command_outcome_unknown::{
    AppendAuthorCommandOutcomeUnknown, AuthorCommandOutcomeUnknownBoundary,
    AuthorCommandOutcomeUnknownError, AuthorCommandOutcomeUnknownObservation,
    AuthorCommandOutcomeUnknownReason, AuthorCommandOutcomeUnknownStore, ReconciliationRequired,
    append_author_command_outcome_unknown,
};

pub use author_edit::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, AuthorEditSettlement,
    AuthorEditSettlementEffect, AuthorEditStore, AuthoritativeAppliedIds, apply_author_edit,
};
pub use author_edit_outcome::{
    ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError, ApplyAuthorEditOutcomeReader,
    ApplyAuthorEditRejectionReason, ApplyAuthorEditUnknownObservation, CommittedApplyAuthorEdit,
    ReadApplyAuthorEditOutcome, get_apply_author_edit_outcome,
};

pub use archive_project::{
    ArchiveProjectCommand, ArchiveProjectError, ArchiveProjectSettlement,
    ArchiveProjectSettlementEffect, ArchiveProjectStore, archive_project,
};
pub use chapter_query::{ChapterQueryFacts, ChapterQueryReader, OpenChapter, open_chapter};
pub use create_chapter::{
    CreateChapterCommand, CreateChapterError, CreateChapterSettlement,
    CreateChapterSettlementEffect, CreateChapterStore, create_chapter,
};
pub use create_project::{
    CreateProjectCommand, CreateProjectError, CreateProjectSettlement, CreateProjectStore,
    create_project,
};
pub use create_project_challenge::{
    CreateProjectChallenge, CreateProjectChallengeBinding, CreateProjectChallengeStore,
    IssueCreateProjectChallenge, issue_create_project_challenge,
};
pub use create_volume::{
    CreateVolumeCommand, CreateVolumeError, CreateVolumeSettlement, CreateVolumeSettlementEffect,
    CreateVolumeStore, create_volume,
};
pub use list_projects::{ProjectLibrary, ProjectLifecycle, ProjectListItem, list_owned_projects};
pub use manuscript_tree::{
    CanonicalManuscriptTree, CanonicalTreeFacts, ChapterFact, ChapterNode, ManuscriptTreeReader,
    VolumeFact, VolumeId, VolumeNode, get_manuscript_tree,
};
pub use update_project::{
    UpdateProjectCommand, UpdateProjectError, UpdateProjectSettlement,
    UpdateProjectSettlementEffect, UpdateProjectStore, update_project,
};
pub use update_volume::{
    UpdateVolumeCommand, UpdateVolumeError, UpdateVolumeSettlement, UpdateVolumeSettlementEffect,
    UpdateVolumeStore, update_volume,
};

pub use editor_session::{
    EditorClientBinding, EditorReadOnlyReason, EditorSession, EditorSessionError, EditorSessionId,
    EditorSessionLookup, EditorSessionSnapshot, EditorSessionStore, EditorWriterState,
    OpenEditorSession, create_editor_session, get_editor_session,
};

pub use takeover::{
    TakeOverProjectWriterCommand, TakeOverProjectWriterEffect, TakeOverProjectWriterError,
    TakeOverProjectWriterSettlement, TakeOverProjectWriterStore, take_over_project_writer,
};

pub use snapshot::{
    AppliedAuthorEditActivity, CanonicalSnapshot, SnapshotLookup, SnapshotReadError, SnapshotStore,
    activity_stream_resume, get_snapshot,
};

pub const PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION: &str =
    "storyos.project-command-challenge-rate.fixed-window.v1";
pub const PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS: u64 = 60;
pub const PROJECT_COMMAND_CHALLENGE_RATE_CAPACITY: i16 = 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserId(String);

impl UserId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for ProjectId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterId(String);

impl ChapterId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for ChapterId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for RevisionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectScope {
    pub owner_user_id: UserId,
    pub project_id: ProjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCommandChallengeBinding {
    pub project_scope: ProjectScope,
    pub client_session_binding_digest: String,
    pub client_session_generation: u64,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub limit_profile_revision: String,
    pub challenge_rate_policy_revision: String,
    pub method: String,
    pub route_template: String,
    pub command_schema: String,
    pub command_kind: String,
    pub canonical_command_digest: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCommandChallenge {
    pub nonce: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueProjectCommandChallenge {
    pub binding: ProjectCommandChallengeBinding,
    pub nonce: String,
    pub nonce_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectCommandChallengeUse {
    FirstUse,
    ExactRetryInProgress,
    ExactRetrySettled { result_reference: String },
}

#[derive(Debug)]
pub enum ProjectCommandChallengeError {
    BindingConflict,
    InvalidOrExpired,
    RateLimited { retry_after_seconds: u64 },
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ProjectCommandChallengeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The idempotency key has another binding"),
            Self::InvalidOrExpired => {
                formatter.write_str("The command challenge is invalid or expired")
            }
            Self::RateLimited { .. } => {
                formatter.write_str("The command challenge rate limit is exceeded")
            }
            Self::Unavailable(_) => {
                formatter.write_str("The command challenge store is unavailable")
            }
        }
    }
}

impl std::error::Error for ProjectCommandChallengeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidOrExpired | Self::RateLimited { .. } => None,
        }
    }
}

/// Owns durable challenge issuance and pre-domain idempotency creation.
pub trait ProjectCommandChallengeStore: Sync {
    fn issue(
        &self,
        request: &IssueProjectCommandChallenge,
    ) -> impl Future<Output = Result<ProjectCommandChallenge, ProjectCommandChallengeError>> + Send;
}

/// Consumes a challenge inside a caller-owned Project command transaction.
///
/// Implementations must not begin or commit a transaction. The caller must use
/// the same transaction for future Admission and authoritative command writes.
pub trait ProjectCommandChallengeTransaction {
    fn consume(
        &mut self,
        binding: &ProjectCommandChallengeBinding,
        nonce_digest: &str,
    ) -> impl Future<Output = Result<ProjectCommandChallengeUse, ProjectCommandChallengeError>> + Send;
}

pub async fn issue_project_command_challenge(
    store: &impl ProjectCommandChallengeStore,
    request: &IssueProjectCommandChallenge,
) -> Result<ProjectCommandChallenge, ProjectCommandChallengeError> {
    store.issue(request).await
}

pub async fn consume_project_command_challenge(
    transaction: &mut impl ProjectCommandChallengeTransaction,
    binding: &ProjectCommandChallengeBinding,
    nonce_digest: &str,
) -> Result<ProjectCommandChallengeUse, ProjectCommandChallengeError> {
    transaction.consume(binding, nonce_digest).await
}

impl ProjectScope {
    pub fn new(owner_user_id: UserId, project_id: ProjectId) -> Self {
        Self {
            owner_user_id,
            project_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub project_id: ProjectId,
    pub title: String,
    pub current_chapter_id: Option<ChapterId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chapter {
    pub chapter_id: ChapterId,
    pub title: String,
    pub revision_id: RevisionId,
    pub body: String,
    pub project_activity_position: u64,
}

#[derive(Debug)]
pub struct ProjectReadError {
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ProjectReadError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }
}

impl std::fmt::Display for ProjectReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Project read is unavailable")
    }
}

impl std::error::Error for ProjectReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Reads canonical Project data under one already authenticated exact Project Scope.
pub trait ProjectReader: Sync {
    fn read_project(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Option<Project>, ProjectReadError>> + Send;

    fn read_chapter(
        &self,
        scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> impl Future<Output = Result<Option<Chapter>, ProjectReadError>> + Send;
}

pub async fn open_project(
    reader: &impl ProjectReader,
    scope: &ProjectScope,
) -> Result<Option<Project>, ProjectReadError> {
    reader.read_project(scope).await
}

pub async fn open_current_chapter(
    reader: &impl ProjectReader,
    scope: &ProjectScope,
    chapter_id: &ChapterId,
) -> Result<Option<Chapter>, ProjectReadError> {
    let Some(project) = reader.read_project(scope).await? else {
        return Ok(None);
    };
    if project.current_chapter_id.as_ref() != Some(chapter_id) {
        return Ok(None);
    }
    reader.read_chapter(scope, chapter_id).await
}

#[cfg(test)]
#[path = "project_read_tests.rs"]
mod tests;
