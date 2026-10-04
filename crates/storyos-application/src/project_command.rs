//! Shared types of project commands that settle through one command sequence.

use storyos_core::TransitionOutcome;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, Project, ProjectCommandChallengeBinding,
    ProjectScope,
};

/// The admitted request facts that every project command carries into its Core Transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCommandEnvelope {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Debug)]
pub enum ProjectCommandError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ProjectCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The command binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => {
                formatter.write_str("The original command acknowledgement cannot be recovered")
            }
            Self::InvalidChallenge => formatter.write_str("The command challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Project store is unavailable"),
        }
    }
}

impl std::error::Error for ProjectCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict
            | Self::HistoricalAcknowledgementUnavailable
            | Self::InvalidChallenge
            | Self::MissingProject => None,
        }
    }
}

/// The settled outcome of one project command, equal on first delivery and replay.
///
/// The applied value `A` is the record of the command's settlement profile, and `P` is the
/// acknowledgement record that the command keeps for an exact retry (ADR 0043).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCommandSettlement<A, N, C, R, P = Project> {
    pub ids: AuthorCommandAdmissionIds,
    pub receipt_created_at: String,
    pub outcome: TransitionOutcome<A, N, C, R>,
    pub response: P,
}

/// The applied value of an `ActivityOnly` command: its effect and its one Activity record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityApplied<A> {
    pub effect: A,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

/// The settled outcome of one Manuscript Structure Transition command.
pub type StructureSettlement<A, N, C, R> = ProjectCommandSettlement<StructureApplied<A>, N, C, R>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureApplied<A> {
    pub effect: A,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
    pub authority: StructureAuthorityEvidence,
}

/// The authority records of one applied command, or their absence before the Authority History Floor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityEvidence<T> {
    Settled(T),
    /// The transition precedes the Authority History Floor and has no Author Action.
    BeforeAuthorityHistoryFloor,
}

impl<T> AuthorityEvidence<T> {
    /// The settled authority, or `None` before the Authority History Floor.
    pub fn into_settled(self) -> Option<T> {
        match self {
            Self::Settled(authority) => Some(authority),
            Self::BeforeAuthorityHistoryFloor => None,
        }
    }
}

pub type StructureAuthorityEvidence = AuthorityEvidence<StructureAuthority>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureAuthority {
    pub authoritative_commit_id: String,
    pub author_action_sequence: u64,
    pub snapshot_id: String,
    pub prior_manuscript_tree_revision: u64,
    pub resulting_manuscript_tree_revision: u64,
    /// The Authoritative Revision that the Commit binds, when it binds one.
    pub resulting_revision_id: Option<String>,
}

/// The applied record of a Current Chapter change (`ChapterSelection` settlement profile).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterSelectionApplied<A> {
    pub effect: A,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
    pub authority: AuthorityEvidence<ChapterSelectionAuthority>,
}

/// The Author Action and canonical Snapshot of a Current Chapter change, which has no Commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterSelectionAuthority {
    pub author_action_sequence: u64,
    pub snapshot_id: String,
    pub manuscript_tree_revision: u64,
}
