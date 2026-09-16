use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ConversationSelection, CreateAgentRunCommand, CreateAgentRunError,
    EditorClientBinding, IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, UpdateProjectAssistanceCommand, UserId, issue_project_command_challenge,
    open_agent_run, request_create_agent_run, update_project_assistance,
};
use storyos_core::AssistanceAvailability;
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-0000000007a2";
const OTHER_PROJECT: &str = "018f0000-0000-7001-8000-0000000007a3";
const CHAPTER: &str = "018f0000-0000-7001-8000-0000000007c1";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const AVAILABLE_BYTES: &[u8] = br#"{"availability":"available"}"#;
const RUN_BYTES: &[u8] = br#"{"conversation":{"kind":"new"}}"#;

fn digest(profile: &str, bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let value = Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        });
    format!("sha256:{profile}:{value}")
}

fn issue_assistance(idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT)),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "PUT".to_owned(),
            route_template: "/api/v1/projects/{project_id}/assistance".to_owned(),
            command_schema: "storyos.command.update-project-assistance.request.v1".to_owned(),
            command_kind: "updateProjectAssistance".to_owned(),
            canonical_command_digest: digest(
                "storyos.command.updateProjectAssistance.jcs.v1",
                AVAILABLE_BYTES,
            ),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn issue_run(idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT)),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/agent-runs".to_owned(),
            command_schema: "storyos.command.create-agent-run.request.v2".to_owned(),
            command_kind: "createAgentRun".to_owned(),
            canonical_command_digest: digest("storyos.command.createAgentRun.jcs.v1", RUN_BYTES),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn run_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    conversation: ConversationSelection,
    conversation_id: &str,
    chapter_id: &str,
) -> CreateAgentRunCommand {
    CreateAgentRunCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: RUN_BYTES.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        conversation,
        author_message: "Help with this passage.".to_owned(),
        chapter_id: chapter_id.to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
        run_id: format!("018f0000-0000-7001-8000-00000004{ids_suffix}"),
        conversation_id: conversation_id.to_owned(),
        project_agent_id: format!("018f0000-0000-7001-8000-00000005{ids_suffix}"),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_agent_run_admits_one_conversation_and_stays_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (mut admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let setup = admin.transaction().await.unwrap();
    setup
        .batch_execute("SET CONSTRAINTS ALL DEFERRED")
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.projects
               (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Assistance Novel', $3::text::uuid)",
            &[&USER_A, &PROJECT, &CHAPTER],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.manuscript_objects
               (owner_user_id, project_id, manuscript_object_id, object_kind, title)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'chapter', 'Assistance Chapter')",
            &[&USER_A, &PROJECT, &CHAPTER],
        )
        .await
        .unwrap();
    setup.commit().await.unwrap();
    admin
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Other Novel', NULL)",
            &[&USER_A, &OTHER_PROJECT],
        )
        .await
        .unwrap();
    let store = PostgresProjectReader::new(runtime_url.clone());
    let scope = ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT));
    let assistance_issue = issue_assistance("7a01");
    issue_project_command_challenge(&store, &assistance_issue)
        .await
        .unwrap();
    update_project_assistance(
        &store,
        &UpdateProjectAssistanceCommand {
            project_scope: assistance_issue.binding.project_scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: assistance_issue
                    .binding
                    .client_session_binding_digest
                    .clone(),
                session_generation: assistance_issue.binding.client_session_generation,
                client_contract_revision: assistance_issue.binding.client_contract_revision.clone(),
                security_policy_revision: assistance_issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: assistance_issue.binding.clone(),
            nonce_digest: assistance_issue.nonce_digest.clone(),
            canonical_command_bytes: AVAILABLE_BYTES.to_vec(),
            correlation_id: "018f0000-0000-7001-8000-000000007a02".to_owned(),
            availability: AssistanceAvailability::Available,
            expected_revision: 0,
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-000000017a02".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-000000027a02".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-000000037a02".to_owned(),
            },
        },
    )
    .await
    .unwrap();

    let first_issue = issue_run("7a11");
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = request_create_agent_run(
        &store,
        &run_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "7a12",
            ConversationSelection::New,
            "018f0000-0000-7001-8000-000000067a12",
            CHAPTER,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        uuid::Uuid::parse_str(&first.memory_settings_revision)
            .expect("settings revision is a UUID")
            .get_version_num(),
        7
    );
    assert_eq!(
        first.conversation_id,
        "018f0000-0000-7001-8000-000000067a12"
    );
    let opened = open_agent_run(&store, &scope, &first.run_id)
        .await
        .unwrap()
        .expect("admitted run");
    assert_eq!(opened.conversation_id, first.conversation_id);
    assert_eq!(
        opened.memory_settings_revision,
        first.memory_settings_revision
    );
    assert_eq!(
        opened.context.record.sufficiency,
        storyos_core::ContextSufficiency::Blocked {
            reasons: vec![storyos_core::ContextBlockReason::WorkingTargetRevisionUnavailable],
        }
    );
    assert_eq!(opened.context.destination_context_manifest_id, None);
    assert_eq!(opened.context.outbound_disclosure_manifest_id, None);
    assert_eq!(
        opened.context.working_target_availability,
        storyos_application::WorkingTargetAvailability::Unavailable
    );
    assert_eq!(
        opened.context.record.destination_io,
        storyos_core::DestinationIo::None
    );
    let grant_id = admin
        .query_one(
            "SELECT run.grant_id::text
               FROM storyos.agent_runs AS run
               JOIN storyos.project_destination_grants AS destination_grant
                 ON (destination_grant.owner_user_id, destination_grant.project_id,
                     destination_grant.grant_id) =
                    (run.owner_user_id, run.project_id, run.grant_id)
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[&USER_A, &PROJECT, &first.run_id],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    assert_eq!(
        uuid::Uuid::parse_str(&grant_id)
            .expect("grant identity is a UUID")
            .get_version_num(),
        7
    );
    let settings = admin
        .query_one(
            "SELECT use_enabled, contribution_enabled
               FROM storyos.conversation_memory_settings
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND conversation_id = $3::text::uuid",
            &[&USER_A, &PROJECT, &first.conversation_id],
        )
        .await
        .unwrap();
    assert_eq!(
        (settings.get::<_, bool>(0), settings.get::<_, bool>(1)),
        (true, true)
    );

    let replay = request_create_agent_run(
        &store,
        &run_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "7a99",
            ConversationSelection::New,
            "018f0000-0000-7001-8000-000000067a99",
            CHAPTER,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let busy_issue = issue_run("7a21");
    issue_project_command_challenge(&store, &busy_issue)
        .await
        .unwrap();
    let busy = request_create_agent_run(
        &store,
        &run_command(
            busy_issue.binding.clone(),
            &busy_issue.nonce_digest,
            "7a22",
            ConversationSelection::Existing {
                conversation_id: first.conversation_id.clone(),
            },
            &first.conversation_id,
            CHAPTER,
        ),
    )
    .await
    .expect_err("queued conversation must stay busy");
    assert!(matches!(busy, CreateAgentRunError::ConversationBusy));

    admin
        .execute(
            "DELETE FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[&USER_A, &PROJECT, &first.run_id],
        )
        .await
        .unwrap();
    let reopen_issue = issue_run("7a25");
    issue_project_command_challenge(&store, &reopen_issue)
        .await
        .unwrap();
    let reopened = request_create_agent_run(
        &store,
        &run_command(
            reopen_issue.binding.clone(),
            &reopen_issue.nonce_digest,
            "7a26",
            ConversationSelection::Existing {
                conversation_id: first.conversation_id.clone(),
            },
            &first.conversation_id,
            CHAPTER,
        ),
    )
    .await
    .unwrap();
    assert_eq!(reopened.conversation_id, first.conversation_id);
    assert_eq!(
        reopened.memory_settings_revision,
        first.memory_settings_revision
    );
    assert_ne!(reopened.run_id, first.run_id);

    let missing_issue = issue_run("7a31");
    issue_project_command_challenge(&store, &missing_issue)
        .await
        .unwrap();
    let missing = request_create_agent_run(
        &store,
        &run_command(
            missing_issue.binding.clone(),
            &missing_issue.nonce_digest,
            "7a32",
            ConversationSelection::Existing {
                conversation_id: "018f0000-0000-7001-8000-00000000dead".to_owned(),
            },
            "018f0000-0000-7001-8000-00000000dead",
            CHAPTER,
        ),
    )
    .await
    .expect_err("unknown conversation must stay inaccessible");
    assert!(matches!(
        missing,
        CreateAgentRunError::InaccessibleConversation
    ));

    let chapter_issue = issue_run("7a41");
    issue_project_command_challenge(&store, &chapter_issue)
        .await
        .unwrap();
    let chapter = request_create_agent_run(
        &store,
        &run_command(
            chapter_issue.binding.clone(),
            &chapter_issue.nonce_digest,
            "7a42",
            ConversationSelection::New,
            "018f0000-0000-7001-8000-000000067a42",
            "018f0000-0000-7001-8000-00000000bad1",
        ),
    )
    .await
    .expect_err("wrong chapter must refuse");
    assert!(matches!(chapter, CreateAgentRunError::InvalidChapterJoin));

    let conversations = admin
        .query_one(
            "SELECT count(*) FROM storyos.project_conversations
              WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    let runs = admin
        .query_one(
            "SELECT count(*) FROM storyos.agent_runs
              WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!((conversations, runs), (1, 1));

    let hidden = open_agent_run(
        &store,
        &ProjectScope::new(UserId::new(USER_B), ProjectId::new(PROJECT)),
        &first.run_id,
    )
    .await
    .unwrap();
    assert_eq!(hidden, None);
}
