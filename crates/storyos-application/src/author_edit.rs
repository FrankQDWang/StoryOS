use storyos_core::{AuthorEditConflict, AuthorEditNoEffect, AuthorEditRefusal, AuthorEditUnit};

use crate::{EditorClientBinding, EditorSessionId, ProjectCommandChallengeBinding, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorCommandAdmissionIds {
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeAppliedIds {
    pub revision_id: String,
    pub payload_id: String,
    pub authoritative_commit_id: String,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Binds an edit to one current Proposal, pending Operation, Revision, and Block.
pub struct AuthorEditProposalTarget {
    pub proposal_id: String,
    pub operation_id: String,
    pub revision_id: String,
    pub manuscript_block_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyAuthorEditCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub writer_generation: u64,
    pub chapter_id: String,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub proposal_target: Option<AuthorEditProposalTarget>,
    pub retry_source: Option<storyos_contracts::DraftRetry>,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub editor_contract_revision: String,
    pub undo_group_id: String,
    pub completed_intent_record_id: String,
    pub local_intent_sequence: u64,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorEditSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: AuthorEditSettlementEffect,
    pub source_draft_disposition: Option<storyos_contracts::SourceDraftDisposition>,
    pub replacement_provenance: Option<storyos_contracts::DraftRetryReplacement>,
    pub receipt_created_at: String,
    pub completed_intent_record_id: String,
    pub local_intent_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditSettlementEffect {
    RefusedToDraft {
        identity: crate::RefusedEditDraftIdentity,
    },
    AuthoritativeApplied {
        ids: AuthoritativeAppliedIds,
        body: String,
        blocks: Vec<crate::ManuscriptBlock>,
        author_action_sequence: u64,
        project_activity_position: u64,
    },
    ProposalRevised {
        proposal_revision_id: String,
        author_action_sequence: u64,
    },
    NoEffect {
        reason: AuthorEditNoEffect,
    },
    Conflicted {
        reason: AuthorEditConflict,
        current_authoritative_revision_id: String,
    },
    Refused {
        reason: AuthorEditRefusal,
    },
}

#[derive(Debug)]
pub enum AuthorEditError {
    BindingConflict,
    InvalidChallenge,
    StaleWriter,
    AdmissionExpired,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for AuthorEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Author Edit binding conflicts"),
            Self::InvalidChallenge => formatter.write_str("The Author Edit challenge is invalid"),
            Self::StaleWriter => {
                formatter.write_str("The Editor Session is not the current writer")
            }
            Self::AdmissionExpired => formatter.write_str("The Author Command Admission expired"),
            Self::Unavailable(_) => formatter.write_str("The Author Edit store is unavailable"),
        }
    }
}

impl std::error::Error for AuthorEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict
            | Self::InvalidChallenge
            | Self::StaleWriter
            | Self::AdmissionExpired => None,
        }
    }
}
