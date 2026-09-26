use std::future::Future;

use crate::{AuthorCommandAdmissionIds, ProjectReadError, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedEditDraftIdentity {
    pub draft_id: String,
    pub draft_revision_id: String,
    pub creation_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedEditDraftRecord {
    pub project_scope: ProjectScope,
    pub identity: RefusedEditDraftIdentity,
    pub payload: storyos_core::RefusedEditPayload,
    pub payload_digest: String,
    pub source: AuthorCommandAdmissionIds,
    pub command_digest: String,
    pub idempotency_key: String,
    pub created_at: String,
    pub closure: String,
    pub retention: String,
    pub closure_event: Option<RefusedEditDraftClosure>,
    pub reopen_event: Option<storyos_contracts::EditorFlowDraftReopened>,
}

/// Reads one retained Draft and its immutable source under the exact Scope.
pub trait RefusedEditDraftReader: Sync {
    fn read_refused_edit_draft(
        &self,
        scope: &ProjectScope,
        draft_id: &str,
    ) -> impl Future<Output = Result<Option<RefusedEditDraftRecord>, ProjectReadError>> + Send;
}

/// A complete source string must fit inside the existing public JSON body ceiling.
pub const AUTHOR_EDIT_INLINE_SOURCE_MAX_BYTES: usize =
    storyos_contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedEditDraftClosure {
    pub event_id: String,
    pub source: AuthorCommandAdmissionIds,
    pub command_digest: String,
    pub idempotency_key: String,
    pub author_action_sequence: String,
    pub created_at: String,
}
