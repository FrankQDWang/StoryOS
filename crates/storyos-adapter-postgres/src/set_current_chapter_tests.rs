use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, CreateChapterCommand, CreateChapterSettlementEffect,
    CreateProjectChallengeBinding, CreateProjectCommand, CreateVolumeCommand, EditorClientBinding,
    EditorSessionId, IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter,
    OpenEditorSession, ProjectCommandChallengeBinding, ProjectId, ProjectScope,
    SetCurrentChapterCommand, SetCurrentChapterSettlementEffect, UserId, create_chapter,
    create_editor_session, create_project, create_volume, issue_create_project_challenge,
    issue_project_command_challenge, open_chapter, open_project, set_current_chapter,
};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";
const CURRENT_BYTES: &[u8] = b"{}";
const CURRENT_DIGEST: &str = "sha256:storyos.command.setCurrentChapter.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
const WRONG_HEAD: &str = "018f0000-0000-7001-8000-00000000ffff";

fn create_project_issue(
    idempotency_key: &str,
    project_suffix: &str,
) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{project_suffix}"
            )),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: format!("{project_suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: format!("sha256:nonce-{project_suffix}"),
    }
}

fn create_project_command(
    binding: CreateProjectChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateProjectCommand {
    let project_scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );
    CreateProjectCommand {
        project_scope,
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: br#"{"title":"Empty Novel"}"#.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: "Empty Novel".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

fn volume_issue(scope: &ProjectScope, idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    command_issue(
        scope,
        idempotency_suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        VOLUME_DIGEST,
    )
}

fn chapter_issue(
    scope: &ProjectScope,
    idempotency_suffix: &str,
    digest: &str,
) -> IssueProjectCommandChallenge {
    command_issue(
        scope,
        idempotency_suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
        "createChapter",
        digest,
    )
}

fn command_issue(
    scope: &ProjectScope,
    idempotency_suffix: &str,
    method: &str,
    route: &str,
    schema: &str,
    kind: &str,
    digest: &str,
) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: method.to_owned(),
            route_template: route.to_owned(),
            command_schema: schema.to_owned(),
            command_kind: kind.to_owned(),
            canonical_command_digest: digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn volume_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateVolumeCommand {
    CreateVolumeCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: client_binding(&binding),
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: VOLUME_BYTES.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: "Volume A".to_owned(),
        expected_tree_revision: 1,
        ids: admission_ids(ids_suffix),
    }
}

fn chapter_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    volume_id: &str,
    title: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> CreateChapterCommand {
    CreateChapterCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: client_binding(&binding),
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        volume_id: volume_id.to_owned(),
        title: title.to_owned(),
        expected_tree_revision,
        ids: admission_ids(ids_suffix),
    }
}

fn current_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    editor_session_id: &str,
    chapter_id: &str,
    expected_current_chapter_id: &str,
    expected_target_revision_id: &str,
) -> SetCurrentChapterCommand {
    SetCurrentChapterCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: client_binding(&binding),
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: CURRENT_BYTES.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        ids: admission_ids(ids_suffix),
        editor_session_id: EditorSessionId::new(editor_session_id),
        chapter_id: chapter_id.to_owned(),
        expected_current_chapter_id: expected_current_chapter_id.to_owned(),
        expected_target_revision_id: expected_target_revision_id.to_owned(),
    }
}

fn client_binding(binding: &ProjectCommandChallengeBinding) -> EditorClientBinding {
    EditorClientBinding {
        binding_ref: binding.client_session_binding_digest.clone(),
        session_generation: binding.client_session_generation,
        client_contract_revision: binding.client_contract_revision.clone(),
        security_policy_revision: binding.security_policy_revision.clone(),
    }
}

fn admission_ids(ids_suffix: &str) -> AuthorCommandAdmissionIds {
    AuthorCommandAdmissionIds {
        command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
        author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
        receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
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
    let issue = create_project_issue("018f0000-0000-7001-8000-000000000b10", "0b10");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0b11"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );
    let volume_issue = volume_issue(&scope, "0b12");
    issue_project_command_challenge(&store, &volume_issue)
        .await
        .unwrap();
    let volume = create_volume(
        &store,
        &volume_command(
            volume_issue.binding.clone(),
            &volume_issue.nonce_digest,
            "0b13",
        ),
    )
    .await
    .unwrap();
    let storyos_application::CreateVolumeSettlementEffect::Applied { volume_id, .. } =
        volume.effect
    else {
        panic!("Create Volume on an empty active Project must apply");
    };
    let first_issue = chapter_issue(&scope, "0b14", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = create_chapter(
        &store,
        &chapter_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0b15",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        chapter_id: chapter_a,
        ..
    } = first.effect
    else {
        panic!("the first Chapter must apply");
    };
    let second_issue = chapter_issue(&scope, "0b16", CHAPTER_B_DIGEST);
    issue_project_command_challenge(&store, &second_issue)
        .await
        .unwrap();
    let second = create_chapter(
        &store,
        &chapter_command(
            second_issue.binding.clone(),
            &second_issue.nonce_digest,
            "0b17",
            &volume_id,
            "Chapter B",
            3,
            CHAPTER_B_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        chapter_id: chapter_b,
        ..
    } = second.effect
    else {
        panic!("the second Chapter must apply");
    };
    let session_issue = command_issue(
        &scope,
        "0b18",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:0b18",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000b19";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-000000000b1a".to_owned(),
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

    let switch_issue = command_issue(
        &scope,
        "0b1b",
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
        "setCurrentChapter",
        CURRENT_DIGEST,
    );
    issue_project_command_challenge(&store, &switch_issue)
        .await
        .unwrap();
    let first_switch = set_current_chapter(
        &store,
        &current_command(
            switch_issue.binding.clone(),
            &switch_issue.nonce_digest,
            "0b1c",
            editor_session_id,
            &chapter_b,
            &chapter_a,
            &revision_b,
        ),
    )
    .await
    .unwrap();
    let SetCurrentChapterSettlementEffect::Applied {
        current_chapter_id, ..
    } = first_switch.effect.clone()
    else {
        panic!("switching to Chapter B must apply");
    };
    assert_eq!(current_chapter_id, chapter_b);
    let replay = set_current_chapter(
        &store,
        &current_command(
            switch_issue.binding.clone(),
            &switch_issue.nonce_digest,
            "0b1d",
            editor_session_id,
            &chapter_b,
            &chapter_a,
            &revision_b,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first_switch);
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_b.clone()))
    );

    let stale_issue = command_issue(
        &scope,
        "0b1e",
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
        "setCurrentChapter",
        CURRENT_DIGEST,
    );
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = set_current_chapter(
        &store,
        &current_command(
            stale_issue.binding,
            &stale_issue.nonce_digest,
            "0b1f",
            editor_session_id,
            &chapter_a,
            &chapter_a,
            &revision_b,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        stale.effect,
        SetCurrentChapterSettlementEffect::Conflicted {
            reason: storyos_core::SetCurrentChapterConflict::StaleCurrentChapter,
        }
    ));

    let wrong_issue = command_issue(
        &scope,
        "0b20",
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
        "setCurrentChapter",
        CURRENT_DIGEST,
    );
    issue_project_command_challenge(&store, &wrong_issue)
        .await
        .unwrap();
    let wrong = set_current_chapter(
        &store,
        &current_command(
            wrong_issue.binding,
            &wrong_issue.nonce_digest,
            "0b21",
            editor_session_id,
            &chapter_a,
            &chapter_b,
            WRONG_HEAD,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        wrong.effect,
        SetCurrentChapterSettlementEffect::Conflicted {
            reason: storyos_core::SetCurrentChapterConflict::WrongTargetHead,
        }
    ));
}
