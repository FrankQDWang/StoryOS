use super::*;
use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditSettlementEffect, ChapterId,
    ChapterNode, CreateChapterCommand, CreateChapterSettlementEffect,
    CreateProjectChallengeBinding, CreateProjectCommand, CreateVolumeCommand,
    CreateVolumePublicOrder, CreateVolumeSettlementEffect, EditorClientBinding, EditorSessionId,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UndoLatestAuthorActionCommand,
    UndoLatestAuthorActionSettlementEffect, UserId, VolumeId, VolumeNode, apply_author_edit,
    create_chapter, create_editor_session, create_project, create_volume, get_manuscript_tree,
    issue_create_project_challenge, issue_project_command_challenge, open_chapter,
    undo_latest_author_action,
};
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_A_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_A_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const VOLUME_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Volume B"}"#;
const CHAPTER_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";

fn create_project_issue(
    owner: &str,
    idempotency_key: &str,
    project_suffix: &str,
) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(owner),
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
    CreateProjectCommand {
        project_scope: ProjectScope::new(
            binding.owner_user_id.clone(),
            binding.prospective_project_id.clone(),
        ),
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

fn command_issue(
    scope: &ProjectScope,
    suffix: &str,
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
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce: format!("opaque-nonce-{suffix}"),
        nonce_digest: format!("sha256:nonce-{suffix}"),
    }
}

async fn seed_project(
    store: &PostgresProjectReader,
    owner: &str,
    project_suffix: &str,
) -> ProjectScope {
    let issue = create_project_issue(
        owner,
        &format!("018f0000-0000-7001-8000-00000000{project_suffix}"),
        project_suffix,
    );
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        store,
        &create_project_command(binding.clone(), &issue.nonce_digest, project_suffix),
    )
    .await
    .unwrap();
    ProjectScope::new(binding.owner_user_id, binding.prospective_project_id)
}

async fn post_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    title: &str,
    bytes: &[u8],
    digest: &str,
    expected_tree_revision: u64,
) -> storyos_application::CreateVolumeSettlement {
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    create_volume(
        store,
        &CreateVolumeCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            title: title.to_owned(),
            expected_tree_revision,
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
        },
    )
    .await
    .unwrap()
}

fn volume_b_digest() -> String {
    format!(
        "sha256:storyos.command.createVolume.jcs.v1:{}",
        crate::author_edit::sha256_hex(VOLUME_B_BYTES)
    )
}

async fn open_admin() -> tokio_postgres::Client {
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn two_users_and_two_projects_keep_separate_structure_sequences() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let first = seed_project(&store, USER_A, "0910").await;
    let second = seed_project(&store, USER_A, "0912").await;
    let other_user = seed_project(&store, USER_B, "0914").await;
    let first_volume = post_volume(
        &store,
        &first,
        "0916",
        "Volume A",
        VOLUME_A_BYTES,
        VOLUME_A_DIGEST,
        1,
    )
    .await;
    let second_volume = post_volume(
        &store,
        &second,
        "0918",
        "Volume A",
        VOLUME_A_BYTES,
        VOLUME_A_DIGEST,
        1,
    )
    .await;
    let other_volume = post_volume(
        &store,
        &other_user,
        "091a",
        "Volume A",
        VOLUME_A_BYTES,
        VOLUME_A_DIGEST,
        1,
    )
    .await;
    let first_authority = first_volume.authority.expect("first Project authority");
    let second_authority = second_volume.authority.expect("second Project authority");
    let other_authority = other_volume.authority.expect("second User authority");
    assert_eq!(first_authority.author_action_sequence, 1);
    assert_eq!(second_authority.author_action_sequence, 1);
    assert_eq!(other_authority.author_action_sequence, 1);
    assert_ne!(
        first_authority.authoritative_commit_id,
        second_authority.authoritative_commit_id
    );
    assert_ne!(
        first_authority.authoritative_commit_id,
        other_authority.authoritative_commit_id
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_create_volume_and_still_reverses_a_later_edit_first() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, USER_A, "0920").await;
    let volume_a = post_volume(
        &store,
        &scope,
        "0922",
        "Volume A",
        VOLUME_A_BYTES,
        VOLUME_A_DIGEST,
        1,
    )
    .await;
    let CreateVolumeSettlementEffect::Applied {
        volume_id: volume_a_id,
        ..
    } = volume_a.effect.clone()
    else {
        panic!("Volume A must apply");
    };
    let chapter_issue = command_issue(
        &scope,
        "0924",
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
        "createChapter",
        CHAPTER_DIGEST,
    );
    issue_project_command_challenge(&store, &chapter_issue)
        .await
        .unwrap();
    let chapter = create_chapter(
        &store,
        &CreateChapterCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: chapter_issue.binding.client_session_binding_digest.clone(),
                session_generation: chapter_issue.binding.client_session_generation,
                client_contract_revision: chapter_issue.binding.client_contract_revision.clone(),
                security_policy_revision: chapter_issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: chapter_issue.binding,
            nonce_digest: chapter_issue.nonce_digest,
            canonical_command_bytes: CHAPTER_BYTES.to_vec(),
            correlation_id: "018f0000-0000-7001-8000-000000000924".to_owned(),
            volume_id: volume_a_id.clone(),
            title: "Chapter A".to_owned(),
            expected_tree_revision: 2,
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-000000010924".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-000000020924".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-000000030924".to_owned(),
            },
        },
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied { chapter_id, .. } = chapter.effect else {
        panic!("Chapter A must apply");
    };
    let session_issue = command_issue(
        &scope,
        "0926",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:0926",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000927";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-000000000928".to_owned(),
            client_binding: EditorClientBinding {
                binding_ref: session_issue.binding.client_session_binding_digest.clone(),
                session_generation: session_issue.binding.client_session_generation,
                client_contract_revision: session_issue.binding.client_contract_revision.clone(),
                security_policy_revision: session_issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: session_issue.binding,
            nonce_digest: session_issue.nonce_digest,
        },
    )
    .await
    .unwrap();
    let OpenChapter::Found(opened) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_id.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter A must open");
    };
    let first_edit = apply_named_edit(
        &store,
        &scope,
        NamedEdit {
            editor_session_id,
            chapter_id: &chapter_id,
            expected_revision_id: opened.chapter.revision_id.as_ref(),
            suffix: "092a",
            local_intent_sequence: 1,
            text: "x",
        },
    )
    .await;
    let volume_b = post_volume(
        &store,
        &scope,
        "092c",
        "Volume B",
        VOLUME_B_BYTES,
        &volume_b_digest(),
        3,
    )
    .await;
    let CreateVolumeSettlementEffect::Applied {
        volume_id: volume_b_id,
        tree_revision,
        order,
    } = volume_b.effect.clone()
    else {
        panic!("Volume B must apply");
    };
    assert_eq!(tree_revision, 4);
    assert_eq!(order, CreateVolumePublicOrder::CanonicalSiblingOrder(2));
    let later_edit = apply_named_edit(
        &store,
        &scope,
        NamedEdit {
            editor_session_id,
            chapter_id: &chapter_id,
            expected_revision_id: match &first_edit.effect {
                AuthorEditSettlementEffect::AuthoritativeApplied { ids, .. } => &ids.revision_id,
                effect => panic!("first edit must apply, received {effect:?}"),
            },
            suffix: "092e",
            local_intent_sequence: 2,
            text: "y",
        },
    )
    .await;
    let AuthorEditSettlementEffect::AuthoritativeApplied {
        ids: later_ids,
        author_action_sequence: later_action,
        ..
    } = later_edit.effect.clone()
    else {
        panic!("later edit must apply");
    };
    let edit_undo = undo_named(
        &store,
        &scope,
        editor_session_id,
        later_action,
        &later_ids.revision_id,
        "0930",
    )
    .await;
    assert!(matches!(
        edit_undo.effect,
        UndoLatestAuthorActionSettlementEffect::Compensated { source_sequence, .. }
            if source_sequence == later_action
    ));
    let tree_after_edit_undo = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the tree remains after prose Undo");
    assert_eq!(tree_after_edit_undo.tree_revision, 4);
    assert_eq!(
        tree_after_edit_undo.volumes[1].volume_id,
        VolumeId::new(volume_b_id.clone())
    );
    let volume_undo = undo_named(
        &store,
        &scope,
        editor_session_id,
        volume_b
            .authority
            .as_ref()
            .expect("Volume B authority")
            .author_action_sequence,
        opened.chapter.revision_id.as_ref(),
        "0932",
    )
    .await;
    let UndoLatestAuthorActionSettlementEffect::CompensatedStructure {
        source_sequence,
        snapshot_id,
        ..
    } = volume_undo.effect.clone()
    else {
        panic!("Create Volume Undo must write structure Compensation");
    };
    assert_eq!(
        source_sequence,
        volume_b
            .authority
            .as_ref()
            .expect("Volume B authority")
            .author_action_sequence
    );
    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the tree remains after structure Compensation");
    assert_eq!(tree.tree_revision, 3);
    assert_eq!(tree.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_a_id),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: vec![ChapterNode {
                chapter_id: ChapterId::new(chapter_id),
                title: "Chapter A".to_owned(),
                order: 1,
            }],
        }]
    );
    let admin = open_admin().await;
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'delete_volume_receipts', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'deleteVolume'
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                               AND compensated_source_sequence = $2::text::numeric
                          ),
                          'forward_delete_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'forward'
                               AND receipt_id IN (
                                 SELECT receipt_id FROM storyos.domain_receipts
                                  WHERE project_id = $1::text::uuid
                                    AND command_kind = 'deleteVolume'
                               )
                          )
                        )::text",
                &[&scope.project_id.as_ref(), &source_sequence.to_string()],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        observed,
        serde_json::json!({
            "delete_volume_receipts": 0,
            "compensation_actions": 1,
            "forward_delete_actions": 0,
        })
    );
}

struct NamedEdit<'a> {
    editor_session_id: &'a str,
    chapter_id: &'a str,
    expected_revision_id: &'a str,
    suffix: &'a str,
    local_intent_sequence: u64,
    text: &'a str,
}

async fn apply_named_edit(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    edit: NamedEdit<'_>,
) -> storyos_application::AuthorEditSettlement {
    let NamedEdit {
        editor_session_id,
        chapter_id,
        expected_revision_id,
        suffix,
        local_intent_sequence,
        text,
    } = edit;
    let mut command = ApplyAuthorEditCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: "sha256:session-a".to_owned(),
            session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
        },
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/manuscript/author-edits".to_owned(),
            command_schema: "storyos.command.apply-author-edit.request.v1".to_owned(),
            command_kind: "applyAuthorEdit".to_owned(),
            canonical_command_digest: String::new(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce_digest: format!("sha256:nonce-{suffix}"),
        canonical_command_bytes: Vec::new(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
        },
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        chapter_id: chapter_id.to_owned(),
        expected_authoritative_revision_id: expected_revision_id.to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        target_refs: vec![format!("manuscript:{chapter_id}")],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: "storyos.editor-contract.release-1.v2".to_owned(),
        undo_group_id: format!("018f0000-0000-7001-8000-00000004{suffix}"),
        completed_intent_record_id: format!("018f0000-0000-7001-8000-00000005{suffix}"),
        local_intent_sequence,
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 0,
                to: 0,
                text: text.to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                from: 0,
                to: 0,
            },
        }],
    };
    let payload = serde_json::json!({
        "command_schema": command.challenge_binding.command_schema,
        "client_contract_revision": command.client_binding.client_contract_revision,
        "security_policy_revision": command.client_binding.security_policy_revision,
        "correlation_id": command.correlation_id,
        "editor_session_id": command.editor_session_id.as_ref(),
        "writer_generation": command.writer_generation.to_string(),
        "chapter_id": command.chapter_id,
        "expected_authoritative_revision_id": command.expected_authoritative_revision_id,
        "expected_proposal_head_revision_ids": command.expected_proposal_head_revision_ids,
        "target_refs": command.target_refs,
        "observed_ownership_partition": command.observed_ownership_partition,
        "editor_contract_revision": command.editor_contract_revision,
        "undo_group_id": command.undo_group_id,
        "completed_intent_record_id": command.completed_intent_record_id,
        "local_intent_sequence": local_intent_sequence.to_string(),
        "author_edit_units": [{
            "normalized_primitives": [{
                "kind": "replace_selection",
                "from": 0,
                "to": 0,
                "text": text,
            }],
            "selection_snapshot": {
                "coordinate_profile": "storyos.editor.utf16-code-unit.v1",
                "from": 0,
                "to": 0,
            },
        }],
    });
    command.canonical_command_bytes = serde_json::to_vec(&payload).unwrap();
    command.challenge_binding.canonical_command_digest = format!(
        "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
        crate::author_edit::sha256_hex(&command.canonical_command_bytes)
    );
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: format!("edit-nonce-{suffix}"),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    apply_author_edit(store, &command).await.unwrap()
}

async fn undo_named(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    editor_session_id: &str,
    expected_frontier: u64,
    expected_revision_id: &str,
    suffix: &str,
) -> storyos_application::UndoLatestAuthorActionSettlement {
    let input = serde_json::json!({
        "expected_author_undo_frontier_sequence": expected_frontier.to_string(),
        "expected_authoritative_revision_id": expected_revision_id,
        "editor_session_id": editor_session_id,
        "client_contract_revision": CLIENT,
        "security_policy_revision": SECURITY,
        "correlation_id": format!("018f0000-0000-7001-8000-00000000{suffix}"),
    });
    let body = serde_json::json!({
        "command_schema": "storyos.command.undo-latest-author-action.request.v1",
        "undo_latest_author_action_input": input,
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let digest = format!(
        "sha256:storyos.command.undoLatestAuthorAction.jcs.v1:{}",
        crate::author_edit::sha256_hex(&bytes)
    );
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/author-actions/undo",
        "storyos.command.undo-latest-author-action.request.v1",
        "undoLatestAuthorAction",
        &digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    undo_latest_author_action(
        store,
        &UndoLatestAuthorActionCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes,
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
            editor_session_id: EditorSessionId::new(editor_session_id),
            expected_author_undo_frontier_sequence: expected_frontier,
            expected_authoritative_revision_id: expected_revision_id.to_owned(),
        },
    )
    .await
    .unwrap()
}
