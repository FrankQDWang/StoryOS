//! StoryOS-owned source for public contract shapes and deterministic generated artifacts.

mod digest;
mod project_command_targets;
mod release1;
mod release1_artifacts;
mod stage1_crosswalk;
mod stage1_delivery;
mod stage1_selection;

pub use release1::{
    AuthoritativeChapterRevision, CREATE_EDITOR_SESSION_METHOD, CREATE_EDITOR_SESSION_PATH,
    CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID, CREATE_PROJECT_COMMAND_CHALLENGE_METHOD,
    CREATE_PROJECT_COMMAND_CHALLENGE_PATH, ControlledProject, CreateEditorSessionRequest,
    CreateEditorSessionResponse, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, CurrentChapter, DigestAlgorithm, DigestValue,
    EditorBaseSnapshot, EditorReadOnlyReason, EditorSessionBinding, EditorWriterProjection,
    GET_CHAPTER_METHOD, GET_CHAPTER_PATH, GET_EDITOR_SESSION_METHOD, GET_EDITOR_SESSION_PATH,
    GET_PROJECT_METHOD, GET_PROJECT_PATH, GET_PROTOCOL_PROFILE_METHOD, GET_PROTOCOL_PROFILE_PATH,
    GetChapterResponse, GetEditorSessionResponse, GetProjectResponse, LIMIT_PROFILE_REVISION,
    ProjectScope, Release1CompatibilityIdentity, Release1ProtocolProfile, StoryOSProblem,
    project_command_kind,
};
pub use release1_artifacts::{
    check_release1_artifacts, release1_protocol_profile, write_release1_artifacts,
};
pub use stage1_crosswalk::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, check_crosswalk, generate_crosswalk, repository_root,
    write_crosswalk,
};
