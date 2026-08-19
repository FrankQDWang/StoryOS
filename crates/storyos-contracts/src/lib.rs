//! StoryOS-owned source for public contract shapes and deterministic generated artifacts.

mod digest;
mod project_command_targets;
mod release1;
mod release1_artifacts;
mod release1_author_edit;
mod release1_author_edit_artifacts;
mod release1_author_edit_outcome;
mod release1_author_edit_outcome_artifacts;
mod release1_snapshot;
mod release1_snapshot_artifacts;
mod release1_takeover;
mod release1_takeover_artifacts;
mod stage1_crosswalk;
mod stage1_delivery;
mod stage1_selection;

pub use release1::{
    APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID, APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
    AUTHOR_EDIT_BATCH_IDLE_MS, AUTHOR_EDIT_BATCH_POLICY_REVISION,
    AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES, AUTHOR_EDIT_MAX_UNITS, AUTHOR_EDIT_MAX_WIRE_BODY_BYTES,
    AuthoritativeChapterRevision, CREATE_EDITOR_SESSION_METHOD, CREATE_EDITOR_SESSION_PATH,
    CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID, CREATE_PROJECT_COMMAND_CHALLENGE_METHOD,
    CREATE_PROJECT_COMMAND_CHALLENGE_PATH, ControlledProject, CreateEditorSessionRequest,
    CreateEditorSessionResponse, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, CurrentChapter, DigestAlgorithm, DigestValue,
    EDITOR_CONTRACT_REVISION, EditorBaseSnapshot, EditorReadOnlyReason, EditorSessionBinding,
    EditorWriterProjection, GET_CHAPTER_METHOD, GET_CHAPTER_PATH, GET_EDITOR_SESSION_METHOD,
    GET_EDITOR_SESSION_PATH, GET_PROJECT_METHOD, GET_PROJECT_PATH, GET_PROTOCOL_PROFILE_METHOD,
    GET_PROTOCOL_PROFILE_PATH, GetChapterResponse, GetEditorSessionResponse, GetProjectResponse,
    LIMIT_PROFILE_REVISION, PUBLIC_PROTOCOL_RELEASE, ProjectScope, Release1CompatibilityIdentity,
    Release1ProtocolProfile, StoryOSProblem, project_command_kind,
};
pub use release1_artifacts::{
    check_release1_artifacts, release1_protocol_profile, write_release1_artifacts,
};
pub use release1_author_edit::{
    APPLY_AUTHOR_EDIT_METHOD, APPLY_AUTHOR_EDIT_PATH, ApplyAuthorEditEffect,
    ApplyAuthorEditRequest, ApplyAuthorEditResponse, AuthorEditConflictReason, AuthorEditPrimitive,
    AuthorEditRefusalReason, AuthorEditUnit, DomainReceipt, DomainReceiptCommandKind,
    DomainReceiptProducerCause, DomainReceiptResult, NoEffectReason, SelectionSnapshot,
};
pub use release1_author_edit_outcome::{
    ApplyAuthorEditOutcome, ApplyAuthorEditRejectionReason, ApplyAuthorEditUnknownObservation,
    GET_APPLY_AUTHOR_EDIT_OUTCOME_METHOD, GET_APPLY_AUTHOR_EDIT_OUTCOME_PATH,
    GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
    GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID, GetApplyAuthorEditOutcomeRequest,
    GetApplyAuthorEditOutcomeResponse, ReconciliationRequired,
};
pub use release1_snapshot::{
    ACTIVITY_STREAM_METHOD, ACTIVITY_STREAM_PATH, ActivityStreamRequest, CanonicalSnapshotMaps,
    GET_SNAPSHOT_METHOD, GET_SNAPSHOT_PATH, GET_SNAPSHOT_REQUEST_SCHEMA_ID,
    GET_SNAPSHOT_RESPONSE_SCHEMA_ID, GetSnapshotRequest, GetSnapshotResponse, SnapshotDescriptor,
    SnapshotKind,
};
pub use release1_takeover::{
    TAKE_OVER_PROJECT_WRITER_METHOD, TAKE_OVER_PROJECT_WRITER_PATH,
    TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID, TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID,
    TakeOverProjectWriterRequest, TakeOverProjectWriterResponse, TakeOverProjectWriterResult,
    TakeoverCompareFailedReason,
};
pub use stage1_crosswalk::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, check_crosswalk, generate_crosswalk, repository_root,
    write_crosswalk,
};
