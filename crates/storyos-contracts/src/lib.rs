//! StoryOS-owned source for public contract shapes and deterministic generated artifacts.

#![recursion_limit = "256"]

mod digest;
mod project_command_targets;
mod release1;
mod release1_archive_project;
mod release1_archive_project_artifacts;
mod release1_artifacts;
mod release1_author_edit;
mod release1_author_edit_artifacts;
mod release1_author_edit_outcome;
mod release1_author_edit_outcome_artifacts;
mod release1_create_chapter;
mod release1_create_chapter_artifacts;
mod release1_create_project;
mod release1_create_project_artifacts;
mod release1_create_volume;
mod release1_create_volume_artifacts;
mod release1_delete_chapter;
mod release1_delete_chapter_artifacts;
mod release1_delete_volume;
mod release1_delete_volume_artifacts;
mod release1_list_projects;
mod release1_list_projects_artifacts;
mod release1_manuscript_tree;
mod release1_manuscript_tree_artifacts;
mod release1_set_current_chapter;
mod release1_set_current_chapter_artifacts;
mod release1_snapshot;
mod release1_snapshot_artifacts;
mod release1_takeover;
mod release1_takeover_artifacts;
mod release1_undo_latest_author_action;
mod release1_undo_latest_author_action_artifacts;
mod release1_update_chapter;
mod release1_update_chapter_artifacts;
mod release1_update_project;
mod release1_update_project_artifacts;
mod release1_update_volume;
mod release1_update_volume_artifacts;
mod stage1_bundle;
mod stage1_crosswalk;
mod stage1_delivery;
mod stage1_handoff;
mod stage1_provenance;
mod stage1_selection;
mod web_assets;

pub use web_assets::{
    RELEASE_1_SECURITY_POLICY_REVISION, WEB_ASSET_MANIFEST, WEB_ASSET_SCHEMA, WebAssetManifest,
    WebResource, web_asset_digest, web_resource_paths, write_web_asset_manifest,
};

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
    LIMIT_PROFILE_REVISION, ManuscriptBlock, ManuscriptBlockKind, PUBLIC_PROTOCOL_RELEASE,
    ProjectOpenState, ProjectScope, Release1CompatibilityIdentity, Release1ProtocolProfile,
    StoryOSProblem, project_command_kind,
};
pub use release1_archive_project::{
    ARCHIVE_PROJECT_DIGEST_PROFILE, ARCHIVE_PROJECT_METHOD, ARCHIVE_PROJECT_PATH,
    ARCHIVE_PROJECT_REQUEST_SCHEMA_ID, ARCHIVE_PROJECT_RESPONSE_SCHEMA_ID,
    ArchiveProjectConflictReason, ArchiveProjectEffect, ArchiveProjectInput,
    ArchiveProjectNoEffectReason, ArchiveProjectRequest, ArchiveProjectResponse,
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
pub use release1_create_chapter::{
    CREATE_CHAPTER_DIGEST_PROFILE, CREATE_CHAPTER_METHOD, CREATE_CHAPTER_PATH,
    CREATE_CHAPTER_REQUEST_SCHEMA_ID, CREATE_CHAPTER_RESPONSE_SCHEMA_ID,
    CreateChapterConflictReason, CreateChapterEffect, CreateChapterInput,
    CreateChapterRefusalReason, CreateChapterRequest, CreateChapterResponse,
};
pub use release1_create_project::{
    CREATE_PROJECT_CHALLENGE_METHOD, CREATE_PROJECT_CHALLENGE_PATH,
    CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID, CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID,
    CREATE_PROJECT_DIGEST_PROFILE, CREATE_PROJECT_METHOD, CREATE_PROJECT_PATH,
    CREATE_PROJECT_REQUEST_SCHEMA_ID, CREATE_PROJECT_RESPONSE_SCHEMA_ID,
    CreateProjectChallengeRequest, CreateProjectChallengeResponse, CreateProjectInput,
    CreateProjectRequest, CreateProjectResponse,
};
pub use release1_create_volume::{
    CREATE_VOLUME_DIGEST_PROFILE, CREATE_VOLUME_METHOD, CREATE_VOLUME_PATH,
    CREATE_VOLUME_REQUEST_SCHEMA_ID, CREATE_VOLUME_RESPONSE_SCHEMA_ID, CreateVolumeConflictReason,
    CreateVolumeEffect, CreateVolumeInput, CreateVolumeRefusalReason, CreateVolumeRequest,
    CreateVolumeResponse,
};
pub use release1_delete_chapter::{
    DELETE_CHAPTER_DIGEST_PROFILE, DELETE_CHAPTER_METHOD, DELETE_CHAPTER_PATH,
    DELETE_CHAPTER_REQUEST_SCHEMA_ID, DELETE_CHAPTER_RESPONSE_SCHEMA_ID,
    DeleteChapterConflictReason, DeleteChapterEffect, DeleteChapterInput,
    DeleteChapterNoEffectReason, DeleteChapterRefusalReason, DeleteChapterRequest,
    DeleteChapterResponse,
};
pub use release1_delete_volume::{
    DELETE_VOLUME_DIGEST_PROFILE, DELETE_VOLUME_METHOD, DELETE_VOLUME_PATH,
    DELETE_VOLUME_REQUEST_SCHEMA_ID, DELETE_VOLUME_RESPONSE_SCHEMA_ID, DeleteVolumeConflictReason,
    DeleteVolumeEffect, DeleteVolumeInput, DeleteVolumeNoEffectReason, DeleteVolumeRefusalReason,
    DeleteVolumeRequest, DeleteVolumeResponse,
};
pub use release1_list_projects::{
    LIST_PROJECTS_METHOD, LIST_PROJECTS_PATH, LIST_PROJECTS_REQUEST_SCHEMA_ID,
    LIST_PROJECTS_RESPONSE_SCHEMA_ID, ListProjectsResponse, ProjectLifecycleState, ProjectListItem,
};
pub use release1_manuscript_tree::{
    GET_MANUSCRIPT_TREE_METHOD, GET_MANUSCRIPT_TREE_PATH, GET_MANUSCRIPT_TREE_REQUEST_SCHEMA_ID,
    GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID, GetManuscriptTreeResponse, ManuscriptChapterNode,
    ManuscriptVolumeNode,
};
pub use release1_set_current_chapter::{
    SET_CURRENT_CHAPTER_DIGEST_PROFILE, SET_CURRENT_CHAPTER_METHOD, SET_CURRENT_CHAPTER_PATH,
    SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID, SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID,
    SetCurrentChapterConflictReason, SetCurrentChapterEffect, SetCurrentChapterInput,
    SetCurrentChapterNoEffectReason, SetCurrentChapterRefusalReason, SetCurrentChapterRequest,
    SetCurrentChapterResponse,
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
pub use release1_undo_latest_author_action::{
    UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE, UNDO_LATEST_AUTHOR_ACTION_METHOD,
    UNDO_LATEST_AUTHOR_ACTION_PATH, UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
    UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID, UndoLatestAuthorActionConflictReason,
    UndoLatestAuthorActionEffect, UndoLatestAuthorActionInput, UndoLatestAuthorActionRequest,
    UndoLatestAuthorActionResponse, UndoLatestAuthorActionUnavailableReason,
};
pub use release1_update_chapter::{
    UPDATE_CHAPTER_DIGEST_PROFILE, UPDATE_CHAPTER_METHOD, UPDATE_CHAPTER_PATH,
    UPDATE_CHAPTER_REQUEST_SCHEMA_ID, UPDATE_CHAPTER_RESPONSE_SCHEMA_ID,
    UpdateChapterConflictReason, UpdateChapterEffect, UpdateChapterInput,
    UpdateChapterNoEffectReason, UpdateChapterRefusalReason, UpdateChapterRequest,
    UpdateChapterResponse,
};
pub use release1_update_project::{
    UPDATE_PROJECT_DIGEST_PROFILE, UPDATE_PROJECT_METHOD, UPDATE_PROJECT_PATH,
    UPDATE_PROJECT_REQUEST_SCHEMA_ID, UPDATE_PROJECT_RESPONSE_SCHEMA_ID,
    UpdateProjectConflictReason, UpdateProjectEffect, UpdateProjectInput,
    UpdateProjectNoEffectReason, UpdateProjectRequest, UpdateProjectResponse,
};
pub use release1_update_volume::{
    UPDATE_VOLUME_DIGEST_PROFILE, UPDATE_VOLUME_METHOD, UPDATE_VOLUME_PATH,
    UPDATE_VOLUME_REQUEST_SCHEMA_ID, UPDATE_VOLUME_RESPONSE_SCHEMA_ID, UpdateVolumeConflictReason,
    UpdateVolumeEffect, UpdateVolumeInput, UpdateVolumeNoEffectReason, UpdateVolumeRefusalReason,
    UpdateVolumeRequest, UpdateVolumeResponse,
};
pub use stage1_crosswalk::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, check_crosswalk, generate_crosswalk, repository_root,
    write_crosswalk,
};
