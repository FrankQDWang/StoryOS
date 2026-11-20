//! The shared test support from `set_current_chapter_authority_tests.rs`.

use crate::shared_tests::command_sequence_structure::{
    CREATE_CHAPTER, CREATE_VOLUME, create_chapter, create_volume,
};
use crate::shared_tests::command_sequence_support::{
    CREATE_EDITOR_SESSION, applied, command_call, command_issue, seed_owned_project,
};
use crate::*;
use storyos_application::{
    ChapterCreated, CreateChapterInput, CreateVolumeInput, EditorClientBinding, EditorSessionId,
    OpenEditorSession, ProjectScope, VolumeCreated, create_editor_session,
    issue_project_command_challenge,
};

pub(crate) const VOLUME_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;

pub(crate) const VOLUME_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";

pub(crate) const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;

pub(crate) const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";

pub(crate) const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;

pub(crate) const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";

pub(crate) async fn post_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
) -> String {
    let issue = command_issue(scope, suffix, &CREATE_VOLUME, VOLUME_DIGEST);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
        store,
        &command_call(
            issue.binding,
            &issue.nonce_digest,
            suffix,
            VOLUME_BYTES,
            CreateVolumeInput {
                title: "Volume A".to_owned(),
                expected_tree_revision: 1,
            },
        ),
    )
    .await
    .unwrap();
    let (VolumeCreated { volume_id, .. }, _) = applied(&settlement);
    volume_id
}

pub(crate) struct SeedChapter<'a> {
    suffix: &'a str,
    title: &'a str,
    bytes: &'a [u8],
    digest: &'a str,
    expected_tree_revision: u64,
}

pub(crate) async fn post_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    volume_id: &str,
    chapter: SeedChapter<'_>,
) -> String {
    let SeedChapter {
        suffix,
        title,
        bytes,
        digest,
        expected_tree_revision,
    } = chapter;
    let issue = command_issue(scope, suffix, &CREATE_CHAPTER, digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_chapter(
        store,
        &command_call(
            issue.binding,
            &issue.nonce_digest,
            suffix,
            bytes,
            CreateChapterInput {
                placement: storyos_core::CreateChapterPlacement::Append,
                volume_id: volume_id.to_owned(),
                title: title.to_owned(),
                expected_tree_revision,
            },
        ),
    )
    .await
    .unwrap();
    let (ChapterCreated { chapter_id, .. }, _) = applied(&settlement);
    chapter_id
}

pub(crate) async fn open_session(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
) -> String {
    let request = open_session_request(store, scope, suffix).await;
    create_editor_session(store, &request).await.unwrap();
    request.editor_session_id.as_ref().to_owned()
}

/// Issues one Create Editor Session Command Challenge and returns the request that uses it.
pub(crate) async fn open_session_request(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
) -> OpenEditorSession {
    let issue = command_issue(
        scope,
        suffix,
        &CREATE_EDITOR_SESSION,
        &format!("sha256:storyos.test:{suffix}"),
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    OpenEditorSession {
        project_scope: scope.clone(),
        editor_session_id: EditorSessionId::new(format!(
            "018f0000-0000-7001-8000-00000001{suffix}"
        )),
        snapshot_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
        client_binding: EditorClientBinding {
            binding_ref: issue.binding.client_session_binding_digest.clone(),
            session_generation: issue.binding.client_session_generation,
            client_contract_revision: issue.binding.client_contract_revision.clone(),
            security_policy_revision: issue.binding.security_policy_revision.clone(),
        },
        challenge_binding: issue.binding,
        nonce_digest: issue.nonce_digest,
    }
}

pub(crate) async fn seed_two_chapters(
    store: &PostgresProjectReader,
    owner: &str,
    project_suffix: &str,
    volume_suffix: &str,
    chapter_a_suffix: &str,
    chapter_b_suffix: &str,
) -> (ProjectScope, String, String, String) {
    let scope = seed_owned_project(store, owner, project_suffix).await;
    let volume_id = post_volume(store, &scope, volume_suffix).await;
    let chapter_a = post_chapter(
        store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: chapter_a_suffix,
            title: "Chapter A",
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
            expected_tree_revision: 2,
        },
    )
    .await;
    let chapter_b = post_chapter(
        store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: chapter_b_suffix,
            title: "Chapter B",
            bytes: CHAPTER_B_BYTES,
            digest: CHAPTER_B_DIGEST,
            expected_tree_revision: 3,
        },
    )
    .await;
    (scope, volume_id, chapter_a, chapter_b)
}
