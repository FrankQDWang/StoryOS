//! The shared test support from `command_sequence_tests/structure_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, applied, command_call, command_issue, issued, seed_project,
    two_chapter_writer,
};
use storyos_application::{ChapterId, EditorSessionId, VolumeId};
use storyos_application::{
    CreateChapterInput, CreateChapterSettlement, CreateVolumeInput, CreateVolumeSettlement,
    DeleteChapterInput, DeleteChapterSettlement, DeleteVolumeInput, DeleteVolumeSettlement,
    ProjectCommandError, ProjectScope, SetCurrentChapterInput, UpdateChapterInput,
    UpdateChapterSettlement, UpdateVolumeInput, UpdateVolumeSettlement,
    issue_project_command_challenge,
};
use storyos_contracts::CommandKind;
use storyos_core::{CreateChapterPlacement, ReasonCode, ReceiptResult, TransitionOutcome};

pub(crate) async fn create_volume(
    store: &PostgresProjectReader,
    call: &CommandCall<CreateVolumeInput>,
) -> Result<CreateVolumeSettlement, ProjectCommandError> {
    store.create_volume(&call.envelope, &call.input).await
}

pub(crate) async fn update_volume(
    store: &PostgresProjectReader,
    call: &CommandCall<UpdateVolumeInput>,
) -> Result<UpdateVolumeSettlement, ProjectCommandError> {
    store.update_volume(&call.envelope, &call.input).await
}

pub(crate) async fn delete_volume(
    store: &PostgresProjectReader,
    call: &CommandCall<DeleteVolumeInput>,
) -> Result<DeleteVolumeSettlement, ProjectCommandError> {
    store.delete_volume(&call.envelope, &call.input).await
}

pub(crate) async fn create_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<CreateChapterInput>,
) -> Result<CreateChapterSettlement, ProjectCommandError> {
    store.create_chapter(&call.envelope, &call.input).await
}

pub(crate) async fn update_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<UpdateChapterInput>,
) -> Result<UpdateChapterSettlement, ProjectCommandError> {
    store.update_chapter(&call.envelope, &call.input).await
}

pub(crate) async fn delete_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<DeleteChapterInput>,
) -> Result<DeleteChapterSettlement, ProjectCommandError> {
    store.delete_chapter(&call.envelope, &call.input).await
}

pub(crate) const CREATE_VOLUME: Route = Route {
    kind: CommandKind::CreateVolume,
    method: storyos_contracts::CREATE_VOLUME_METHOD,
    path: storyos_contracts::CREATE_VOLUME_PATH,
    schema: storyos_contracts::CREATE_VOLUME_REQUEST_SCHEMA_ID,
};

pub(crate) const UPDATE_VOLUME: Route = Route {
    kind: CommandKind::UpdateVolume,
    method: storyos_contracts::UPDATE_VOLUME_METHOD,
    path: storyos_contracts::UPDATE_VOLUME_PATH,
    schema: storyos_contracts::UPDATE_VOLUME_REQUEST_SCHEMA_ID,
};

pub(crate) const DELETE_VOLUME: Route = Route {
    kind: CommandKind::DeleteVolume,
    method: storyos_contracts::DELETE_VOLUME_METHOD,
    path: storyos_contracts::DELETE_VOLUME_PATH,
    schema: storyos_contracts::DELETE_VOLUME_REQUEST_SCHEMA_ID,
};

pub(crate) const CREATE_CHAPTER: Route = Route {
    kind: CommandKind::CreateChapter,
    method: storyos_contracts::CREATE_CHAPTER_METHOD,
    path: storyos_contracts::CREATE_CHAPTER_PATH,
    schema: storyos_contracts::CREATE_CHAPTER_REQUEST_SCHEMA_ID,
};

pub(crate) const UPDATE_CHAPTER: Route = Route {
    kind: CommandKind::UpdateChapter,
    method: storyos_contracts::UPDATE_CHAPTER_METHOD,
    path: storyos_contracts::UPDATE_CHAPTER_PATH,
    schema: storyos_contracts::UPDATE_CHAPTER_REQUEST_SCHEMA_ID,
};

pub(crate) const SET_CURRENT_CHAPTER: Route = Route {
    kind: CommandKind::SetCurrentChapter,
    method: storyos_contracts::SET_CURRENT_CHAPTER_METHOD,
    path: storyos_contracts::SET_CURRENT_CHAPTER_PATH,
    schema: storyos_contracts::SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID,
};

pub(crate) const DELETE_CHAPTER: Route = Route {
    kind: CommandKind::DeleteChapter,
    method: storyos_contracts::DELETE_CHAPTER_METHOD,
    path: storyos_contracts::DELETE_CHAPTER_PATH,
    schema: storyos_contracts::DELETE_CHAPTER_REQUEST_SCHEMA_ID,
};

pub(crate) async fn new_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    expected_tree_revision: u64,
) -> VolumeId {
    let input = CreateVolumeInput {
        title: format!("Volume {suffix:04x}"),
        expected_tree_revision,
    };
    let call = issued(store, scope, suffix, &CREATE_VOLUME, input).await;
    VolumeId::new(
        applied(&create_volume(store, &call).await.unwrap())
            .0
            .volume_id,
    )
}

pub(crate) async fn new_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    volume_id: &VolumeId,
    expected_tree_revision: u64,
) -> ChapterId {
    let input = CreateChapterInput {
        volume_id: volume_id.as_ref().to_owned(),
        title: format!("Chapter {suffix:04x}"),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision,
    };
    let call = issued(store, scope, suffix, &CREATE_CHAPTER, input).await;
    ChapterId::new(
        applied(&create_chapter(store, &call).await.unwrap())
            .0
            .chapter_id,
    )
}

/// Settles one applied Create Volume of `title` whose ids derive from the four hex digits
/// `suffix`, and returns the Volume.
pub(crate) async fn titled_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    title: &str,
    expected_tree_revision: u64,
) -> String {
    let input = CreateVolumeInput {
        title: title.to_owned(),
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &CREATE_VOLUME, input).await;
    applied(&create_volume(store, &call).await.unwrap())
        .0
        .volume_id
}

/// Settles one applied Create Chapter of `title` at the end of `volume_id`, whose ids derive from
/// the four hex digits `suffix`, and returns the Chapter.
pub(crate) async fn titled_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    title: &str,
    expected_tree_revision: u64,
) -> String {
    let input = CreateChapterInput {
        volume_id: volume_id.to_owned(),
        title: title.to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &CREATE_CHAPTER, input).await;
    applied(&create_chapter(store, &call).await.unwrap())
        .0
        .chapter_id
}

/// Issues one Command Challenge for `route` whose ids derive from the four hex digits `suffix`,
/// and binds `input` to it.
pub(crate) async fn named_call<I>(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    route: &Route,
    input: I,
) -> CommandCall<I> {
    let issue = command_issue(
        scope,
        suffix,
        route,
        &format!("sha256:{}:{suffix}", route.kind.as_str()),
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    command_call(issue.binding, &issue.nonce_digest, suffix, b"{}", input)
}

/// The result kind and the reason code of one outcome. An applied outcome has no reason code.
pub(crate) fn outcome_code<A, N: ReasonCode, C: ReasonCode, R: ReasonCode>(
    outcome: &TransitionOutcome<A, N, C, R>,
) -> (ReceiptResult, Option<&'static str>) {
    let code = match outcome {
        TransitionOutcome::Applied(_) => None,
        TransitionOutcome::NoEffect(reason) => Some(reason.code()),
        TransitionOutcome::Conflicted(reason) => Some(reason.code()),
        TransitionOutcome::Refused(reason) => Some(reason.code()),
    };
    (outcome.receipt_result(), code)
}

/// One applicable Set Current Chapter from Chapter A to Chapter B in a new Project.
pub(crate) async fn set_current_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<SetCurrentChapterInput> {
    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let input = SetCurrentChapterInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        chapter_id: chapter_b,
        expected_current_chapter_id: chapter_a,
        expected_target_revision_id: revision_b,
    };
    issued(store, &scope, base + 9, &SET_CURRENT_CHAPTER, input).await
}

/// One applicable Create Volume in a new Project.
pub(crate) async fn create_volume_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<CreateVolumeInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = CreateVolumeInput {
        title: "Volume".to_owned(),
        expected_tree_revision: 1,
    };
    issued(store, &scope, base + 9, &CREATE_VOLUME, input).await
}

/// One applicable Update Volume in a new Project with one Volume.
pub(crate) async fn update_volume_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateVolumeInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, /*expected_tree_revision*/ 1).await;
    let input = UpdateVolumeInput {
        volume_id,
        title: "Renamed".to_owned(),
        order: 1,
        expected_tree_revision: 2,
    };
    issued(store, &scope, base + 9, &UPDATE_VOLUME, input).await
}

/// One applicable Delete Volume in a new Project with one empty Volume.
pub(crate) async fn delete_volume_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<DeleteVolumeInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, /*expected_tree_revision*/ 1).await;
    let input = DeleteVolumeInput {
        volume_id,
        expected_tree_revision: 2,
    };
    issued(store, &scope, base + 9, &DELETE_VOLUME, input).await
}

/// One applicable Create Chapter in a new Project with one Volume.
pub(crate) async fn create_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<CreateChapterInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, /*expected_tree_revision*/ 1).await;
    let input = CreateChapterInput {
        volume_id: volume_id.as_ref().to_owned(),
        title: "Chapter".to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision: 2,
    };
    issued(store, &scope, base + 9, &CREATE_CHAPTER, input).await
}

/// One applicable Update Chapter in a new Project with one Volume and one Chapter.
pub(crate) async fn update_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateChapterInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, /*expected_tree_revision*/ 1).await;
    let chapter_id = new_chapter(
        store,
        &scope,
        base + 2,
        &volume_id,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let input = UpdateChapterInput {
        chapter_id,
        title: "Renamed".to_owned(),
        order: 1,
        expected_tree_revision: 3,
    };
    issued(store, &scope, base + 9, &UPDATE_CHAPTER, input).await
}

/// One applicable Delete Chapter in a new Project with one Volume and one Chapter.
pub(crate) async fn delete_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<DeleteChapterInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, /*expected_tree_revision*/ 1).await;
    let chapter_id = new_chapter(
        store,
        &scope,
        base + 2,
        &volume_id,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let input = DeleteChapterInput {
        chapter_id,
        expected_tree_revision: 3,
    };
    issued(store, &scope, base + 9, &DELETE_CHAPTER, input).await
}
