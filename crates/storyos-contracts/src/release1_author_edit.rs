use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{
    APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID, APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
    AuthoritativeChapterRevision, DigestValue, ProjectScope, QueryOperation,
};

pub(super) const APPLY_AUTHOR_EDIT: QueryOperation = QueryOperation {
    operation_id: "applyAuthorEdit",
    method: "POST",
    path: "/api/v1/projects/{project_id}/manuscript/author-edits",
    request_schema: APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID,
    response_schema: APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Committed author edit"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or authoritative Head conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Author edit refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.applyAuthorEdit.positive.v1",
        "storyos.golden.applyAuthorEdit.invalid.v1",
        "storyos.golden.applyAuthorEdit.boundary.v1",
    ],
};

pub const APPLY_AUTHOR_EDIT_PATH: &str = APPLY_AUTHOR_EDIT.path;
pub const APPLY_AUTHOR_EDIT_METHOD: &str = APPLY_AUTHOR_EDIT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ApplyAuthorEditRequest {
    pub command_schema: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
    pub editor_session_id: String,
    pub writer_generation: String,
    pub chapter_id: String,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub editor_contract_revision: String,
    pub undo_group_id: String,
    pub completed_intent_record_id: String,
    pub local_intent_sequence: String,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AuthorEditUnit {
    pub normalized_primitives: Vec<AuthorEditPrimitive>,
    pub selection_snapshot: SelectionSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthorEditPrimitive {
    ReplaceSelection {
        from: u32,
        to: u32,
        text: String,
    },
    ReplaceBlockSelection {
        manuscript_block_id: String,
        from: u32,
        to: u32,
        text: String,
    },
    SplitBlock {
        manuscript_block_id: String,
        offset: u32,
        new_manuscript_block_id: String,
    },
    JoinBlocks {
        left_manuscript_block_id: String,
        right_manuscript_block_id: String,
    },
    MoveBlock {
        manuscript_block_id: String,
        to_index: u32,
    },
    RetypeBlock {
        manuscript_block_id: String,
        block_kind: crate::ManuscriptBlockKind,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectionSnapshot {
    pub coordinate_profile: String,
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DomainReceiptProducerCause {
    AuthorCommandAdmission,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub enum DomainReceiptCommandKind {
    #[serde(rename = "applyAuthorEdit")]
    #[ts(rename = "applyAuthorEdit")]
    ApplyAuthorEdit,
    #[serde(rename = "takeOverProjectWriter")]
    #[ts(rename = "takeOverProjectWriter")]
    TakeOverProjectWriter,
    #[serde(rename = "createProject")]
    #[ts(rename = "createProject")]
    CreateProject,
    #[serde(rename = "updateProject")]
    #[ts(rename = "updateProject")]
    UpdateProject,
    #[serde(rename = "archiveProject")]
    #[ts(rename = "archiveProject")]
    ArchiveProject,
    #[serde(rename = "createVolume")]
    #[ts(rename = "createVolume")]
    CreateVolume,
    #[serde(rename = "createChapter")]
    #[ts(rename = "createChapter")]
    CreateChapter,
    #[serde(rename = "updateVolume")]
    #[ts(rename = "updateVolume")]
    UpdateVolume,
    #[serde(rename = "updateChapter")]
    #[ts(rename = "updateChapter")]
    UpdateChapter,
    #[serde(rename = "setCurrentChapter")]
    #[ts(rename = "setCurrentChapter")]
    SetCurrentChapter,
    #[serde(rename = "undoLatestAuthorAction")]
    #[ts(rename = "undoLatestAuthorAction")]
    UndoLatestAuthorAction,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DomainReceiptResult {
    AuthoritativeApplied,
    NoEffect,
    Conflicted,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DomainReceipt {
    pub receipt_id: String,
    pub project_scope: ProjectScope,
    pub command_kind: DomainReceiptCommandKind,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub producer_cause: DomainReceiptProducerCause,
    pub author_command_admission_id: String,
    pub expected_heads: Vec<String>,
    pub prior_heads: Vec<String>,
    pub resulting_heads: Vec<String>,
    pub authoritative_revision_ids: Vec<String>,
    pub proposal_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    #[schemars(required)]
    pub author_action_sequence: Option<String>,
    pub draft_artifact_refs: Vec<String>,
    pub artifact_lifecycle_event_refs: Vec<String>,
    pub condition_refs: Vec<String>,
    pub result: DomainReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NoEffectReason {
    ContentUnchanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuthorEditConflictReason {
    StaleAuthoritativeHead,
    ProposalHeadPresent,
    OwnershipChanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuthorEditRefusalReason {
    UnsupportedIntentShape,
    InvalidSelection,
    TargetMismatch,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplyAuthorEditEffect {
    AuthoritativeApplied {
        authoritative_revision: AuthoritativeChapterRevision,
        authoritative_commit_id: String,
        author_action_sequence: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: NoEffectReason,
    },
    Conflicted {
        reason: AuthorEditConflictReason,
        current_authoritative_revision_id: String,
    },
    Refused {
        reason: AuthorEditRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ApplyAuthorEditResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub effect: ApplyAuthorEditEffect,
    pub completed_intent_record_id: String,
    pub local_intent_sequence: String,
}
