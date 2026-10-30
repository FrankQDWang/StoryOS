use super::*;
use storyos_application::{
    ChapterCreated, ChapterId, CreateChapterInput, CreateVolumeInput, EditorClientBinding,
    EditorSessionId, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectScope, SetCurrentChapterInput, VolumeCreated,
    create_editor_session, create_project, issue_create_project_challenge,
    issue_project_command_challenge, open_chapter, open_project,
};
use storyos_core::TransitionOutcome;

use crate::command_sequence::tests::{
    CREATE_CHAPTER, CREATE_EDITOR_SESSION, CREATE_VOLUME, CommandCall, SET_CURRENT_CHAPTER,
    applied, command_call, command_issue, create_chapter, create_project_command,
    create_project_issue, create_volume,
};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const VOLUME_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";
const CURRENT_BYTES: &[u8] = b"{}";
const CURRENT_DIGEST: &str = "sha256:storyos.command.setCurrentChapter.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
const WRONG_HEAD: &str = "018f0000-0000-7001-8000-00000000ffff";

fn chapter_issue(
    scope: &ProjectScope,
    idempotency_suffix: &str,
    digest: &str,
) -> IssueProjectCommandChallenge {
    command_issue(scope, idempotency_suffix, &CREATE_CHAPTER, digest)
}

fn chapter_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    volume_id: &str,
    title: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> CommandCall<CreateChapterInput> {
    command_call(
        binding,
        nonce_digest,
        ids_suffix,
        bytes,
        CreateChapterInput {
            placement: storyos_core::CreateChapterPlacement::Append,
            volume_id: volume_id.to_owned(),
            title: title.to_owned(),
            expected_tree_revision,
        },
    )
}

fn current_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    editor_session_id: &str,
    chapter_id: &str,
    expected_current_chapter_id: &str,
    expected_target_revision_id: &str,
) -> CommandCall<SetCurrentChapterInput> {
    command_call(
        binding,
        nonce_digest,
        ids_suffix,
        CURRENT_BYTES,
        SetCurrentChapterInput {
            editor_session_id: EditorSessionId::new(editor_session_id),
            chapter_id: chapter_id.to_owned(),
            expected_current_chapter_id: expected_current_chapter_id.to_owned(),
            expected_target_revision_id: expected_target_revision_id.to_owned(),
        },
    )
}

async fn set_current_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<SetCurrentChapterInput>,
) -> Result<
    storyos_application::SetCurrentChapterSettlement,
    storyos_application::ProjectCommandError,
> {
    store.set_current_chapter(&call.envelope, &call.input).await
}

fn client_binding(binding: &ProjectCommandChallengeBinding) -> EditorClientBinding {
    EditorClientBinding {
        binding_ref: binding.client_session_binding_digest.clone(),
        session_generation: binding.client_session_generation,
        client_contract_revision: binding.client_contract_revision.clone(),
        security_policy_revision: binding.security_policy_revision.clone(),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn set_current_chapter_is_atomic_replayable_and_fail_closed() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue(USER_A, "018f0000-0000-7001-8000-000000000e10", "0e10");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0e11"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );
    let volume_issue = command_issue(&scope, "0e12", &CREATE_VOLUME, VOLUME_DIGEST);
    issue_project_command_challenge(&store, &volume_issue)
        .await
        .unwrap();
    let volume = create_volume(
        &store,
        &command_call(
            volume_issue.binding.clone(),
            &volume_issue.nonce_digest,
            "0e13",
            VOLUME_BYTES,
            CreateVolumeInput {
                title: "Volume A".to_owned(),
                expected_tree_revision: 1,
            },
        ),
    )
    .await
    .unwrap();
    let (VolumeCreated { volume_id, .. }, _) = applied(&volume);
    let first_issue = chapter_issue(&scope, "0e14", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = create_chapter(
        &store,
        &chapter_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0e15",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    let (
        ChapterCreated {
            chapter_id: chapter_a,
            ..
        },
        _,
    ) = applied(&first);
    let second_issue = chapter_issue(&scope, "0e16", CHAPTER_B_DIGEST);
    issue_project_command_challenge(&store, &second_issue)
        .await
        .unwrap();
    let second = create_chapter(
        &store,
        &chapter_command(
            second_issue.binding.clone(),
            &second_issue.nonce_digest,
            "0e17",
            &volume_id,
            "Chapter B",
            3,
            CHAPTER_B_BYTES,
        ),
    )
    .await
    .unwrap();
    let (
        ChapterCreated {
            chapter_id: chapter_b,
            ..
        },
        _,
    ) = applied(&second);
    let session_issue = command_issue(
        &scope,
        "0e18",
        &CREATE_EDITOR_SESSION,
        "sha256:storyos.test:0e18",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000e19";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-000000000e1a".to_owned(),
            client_binding: client_binding(&session_issue.binding),
            challenge_binding: session_issue.binding,
            nonce_digest: session_issue.nonce_digest,
        },
    )
    .await
    .unwrap();
    let OpenChapter::Found(opened_b) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter B must open");
    };
    let revision_b = opened_b.chapter.revision_id.as_ref().to_owned();

    let switch_issue = command_issue(&scope, "0e1b", &SET_CURRENT_CHAPTER, CURRENT_DIGEST);
    issue_project_command_challenge(&store, &switch_issue)
        .await
        .unwrap();
    let first_switch = set_current_chapter(
        &store,
        &current_command(
            switch_issue.binding.clone(),
            &switch_issue.nonce_digest,
            "0e1c",
            editor_session_id,
            &chapter_b,
            &chapter_a,
            &revision_b,
        ),
    )
    .await
    .unwrap();
    let TransitionOutcome::Applied(applied) = &first_switch.outcome else {
        panic!("switching to Chapter B must apply");
    };
    assert_eq!(applied.effect.current_chapter_id, chapter_b);
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_b.clone()))
    );

    let stale_issue = command_issue(&scope, "0e1e", &SET_CURRENT_CHAPTER, CURRENT_DIGEST);
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = set_current_chapter(
        &store,
        &current_command(
            stale_issue.binding,
            &stale_issue.nonce_digest,
            "0e1f",
            editor_session_id,
            &chapter_a,
            &chapter_a,
            &revision_b,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        stale.outcome,
        TransitionOutcome::Conflicted(storyos_core::SetCurrentChapterConflict::StaleCurrentChapter)
    ));

    let wrong_issue = command_issue(&scope, "0e20", &SET_CURRENT_CHAPTER, CURRENT_DIGEST);
    issue_project_command_challenge(&store, &wrong_issue)
        .await
        .unwrap();
    let wrong = set_current_chapter(
        &store,
        &current_command(
            wrong_issue.binding,
            &wrong_issue.nonce_digest,
            "0e21",
            editor_session_id,
            &chapter_a,
            &chapter_b,
            WRONG_HEAD,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        wrong.outcome,
        TransitionOutcome::Conflicted(storyos_core::SetCurrentChapterConflict::WrongTargetHead)
    ));
}
