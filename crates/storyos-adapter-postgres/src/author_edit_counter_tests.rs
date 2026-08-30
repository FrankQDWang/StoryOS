use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditError, AuthorEditSettlementEffect, EditorClientBinding,
    EditorSessionId, IssueProjectCommandChallenge, OpenEditorSession, ProjectId, ProjectScope,
    UserId, create_editor_session, issue_project_command_challenge,
};
use storyos_core::AuthorEditPrimitive;
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

use super::tests::{AUTHOR_EDIT_TEST_LOCK, author_command, bind_canonical_payload, binding};
use super::*;

const MAX_COUNTER: &str = "18446744073709551614";

#[derive(Debug, Eq, PartialEq)]
struct SettlementOwnedState {
    counters: (String, String, String),
    head_revision_id: String,
    body: String,
    latest_commit_sequence: String,
    receipt_count: i64,
    revision_count: i64,
    commit_count: i64,
    author_action_count: i64,
    activity_count: i64,
    project_snapshot_count: i64,
    base_snapshot_count: i64,
    settlement_count: i64,
}

async fn read_settlement_owned_state(
    admin: &Client,
    owner_user_id: &str,
    project_id: &str,
    chapter_id: &str,
) -> SettlementOwnedState {
    let row = admin
        .query_one(
            "SELECT counters.author_action_sequence::text,
                    counters.authoritative_commit_sequence::text,
                    counters.project_activity_position::text,
                    head.current_revision_id::text,
                    convert_from(payload.canonical_bytes, 'UTF8'),
                    (SELECT max(authoritative_commit_sequence)::text
                       FROM storyos.authoritative_commits
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_revisions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.project_activity_events
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.project_snapshots
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.editor_session_base_snapshots
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.author_command_admission_settlements
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid)
               FROM storyos.scope_counters AS counters
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id) =
                    (counters.owner_user_id, counters.project_id)
                AND head.manuscript_object_id = $3::text::uuid
               JOIN storyos.authoritative_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id,
                     revision.manuscript_object_id, revision.revision_id) =
                    (head.owner_user_id, head.project_id,
                     head.manuscript_object_id, head.current_revision_id)
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
              WHERE counters.owner_user_id = $1::text::uuid
                AND counters.project_id = $2::text::uuid",
            &[&owner_user_id, &project_id, &chapter_id],
        )
        .await
        .unwrap();
    SettlementOwnedState {
        counters: (row.get(0), row.get(1), row.get(2)),
        head_revision_id: row.get(3),
        body: row.get(4),
        latest_commit_sequence: row.get(5),
        receipt_count: row.get(6),
        revision_count: row.get(7),
        commit_count: row.get(8),
        author_action_count: row.get(9),
        activity_count: row.get(10),
        project_snapshot_count: row.get(11),
        base_snapshot_count: row.get(12),
        settlement_count: row.get(13),
    }
}

async fn revision_member_block_id(
    admin: &Client,
    owner_user_id: &str,
    project_id: &str,
    chapter_id: &str,
    revision_id: &str,
) -> String {
    admin
        .query_one(
            "SELECT manuscript_block_id::text
               FROM storyos.manuscript_revision_members
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid
                AND revision_id = $4::text::uuid",
            &[&owner_user_id, &project_id, &chapter_id, &revision_id],
        )
        .await
        .unwrap()
        .get(0)
}

fn set_unique_command_identity(command: &mut ApplyAuthorEditCommand, chapter_id: &str) {
    command.correlation_id = Uuid::now_v7().to_string();
    command.ids.command_id = Uuid::now_v7().to_string();
    command.ids.author_command_admission_id = Uuid::now_v7().to_string();
    command.ids.receipt_id = Uuid::now_v7().to_string();
    command.chapter_id = chapter_id.to_owned();
    command.target_refs = vec![format!("manuscript:{chapter_id}")];
    command.undo_group_id = Uuid::now_v7().to_string();
    command.completed_intent_record_id = Uuid::now_v7().to_string();
}

fn prepare_follow_up(
    command: &mut ApplyAuthorEditCommand,
    expected_revision_id: &str,
    position: u32,
    replacement: &str,
) {
    command.expected_authoritative_revision_id = expected_revision_id.to_owned();
    let [insert, replace] = command.author_edit_units.as_mut_slice() else {
        panic!("counter test command must contain two units")
    };
    match &mut insert.normalized_primitives[0] {
        AuthorEditPrimitive::ReplaceSelection { from, to, text } => {
            *from = position;
            *to = position;
            *text = "x".to_owned();
        }
        AuthorEditPrimitive::ReplaceBlockSelection { .. }
        | AuthorEditPrimitive::SplitBlock { .. }
        | AuthorEditPrimitive::JoinBlocks { .. }
        | AuthorEditPrimitive::MoveBlock { .. }
        | AuthorEditPrimitive::RetypeBlock { .. } => {
            panic!("counter test command must use ReplaceSelection")
        }
    }
    insert.selection_snapshot.from = position;
    insert.selection_snapshot.to = position;
    match &mut replace.normalized_primitives[0] {
        AuthorEditPrimitive::ReplaceSelection { from, to, text } => {
            *from = position;
            *to = position + 1;
            *text = replacement.to_owned();
        }
        AuthorEditPrimitive::ReplaceBlockSelection { .. }
        | AuthorEditPrimitive::SplitBlock { .. }
        | AuthorEditPrimitive::JoinBlocks { .. }
        | AuthorEditPrimitive::MoveBlock { .. }
        | AuthorEditPrimitive::RetypeBlock { .. } => {
            panic!("counter test command must use ReplaceSelection")
        }
    }
    replace.selection_snapshot.from = position;
    replace.selection_snapshot.to = position + 1;
    bind_canonical_payload(command);
}

async fn issue_command_challenge(
    store: &PostgresProjectReader,
    command: &ApplyAuthorEditCommand,
    nonce: &str,
) {
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: nonce.to_owned(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_edit_counters_cover_missing_existing_and_limit_rows() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(&runtime_url);
    let (mut admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });

    let owner_user_id = Uuid::now_v7().to_string();
    let project_id = Uuid::now_v7().to_string();
    let chapter_id = Uuid::now_v7().to_string();
    let payload_id = Uuid::now_v7().to_string();
    let initial_revision_id = Uuid::now_v7().to_string();
    let setup = admin.transaction().await.unwrap();
    setup
        .batch_execute("SET CONSTRAINTS ALL DEFERRED")
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.users (user_id) VALUES ($1::text::uuid)",
            &[&owner_user_id],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.projects
               (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Counter Project', $3::text::uuid)",
            &[&owner_user_id, &project_id, &chapter_id],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.manuscript_objects
               (owner_user_id, project_id, manuscript_object_id, object_kind, title)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'chapter', 'Counter Chapter')",
            &[&owner_user_id, &project_id, &chapter_id],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     convert_to('Authoritative A', 'UTF8'))",
            &[&owner_user_id, &project_id, &payload_id],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     $4::text::uuid, $5::text::uuid)",
            &[
                &owner_user_id,
                &project_id,
                &chapter_id,
                &initial_revision_id,
                &payload_id,
            ],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.authoritative_heads
               (owner_user_id, project_id, manuscript_object_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[
                &owner_user_id,
                &project_id,
                &chapter_id,
                &initial_revision_id,
            ],
        )
        .await
        .unwrap();
    setup.commit().await.unwrap();

    let scope = ProjectScope::new(UserId::new(&owner_user_id), ProjectId::new(&project_id));
    let editor_session_id = Uuid::now_v7().to_string();
    let session_nonce_digest = "sha256:counter-session";
    let session_binding = binding(
        &scope,
        "createEditorSession",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        &Uuid::now_v7().to_string(),
    );
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: session_binding.clone(),
            nonce: "counter-session-nonce".to_owned(),
            nonce_digest: session_nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(&editor_session_id),
            snapshot_id: Uuid::now_v7().to_string(),
            client_binding: EditorClientBinding {
                binding_ref: "binding:author-edit".to_owned(),
                session_generation: 1,
                client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
                security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            },
            challenge_binding: session_binding,
            nonce_digest: session_nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();

    let missing = admin
        .query_one(
            "SELECT NOT EXISTS (
               SELECT 1 FROM storyos.scope_counters
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid)",
            &[&owner_user_id, &project_id],
        )
        .await
        .unwrap()
        .get::<_, bool>(0);
    assert!(missing);

    let mut first_command = author_command(
        &scope,
        &editor_session_id,
        &Uuid::now_v7().to_string(),
        "sha256:counter-first",
        "91",
    );
    set_unique_command_identity(&mut first_command, &chapter_id);
    first_command.expected_authoritative_revision_id = initial_revision_id.clone();
    bind_canonical_payload(&mut first_command);
    issue_command_challenge(&store, &first_command, "counter-first-nonce").await;
    let first = storyos_application::apply_author_edit(&store, &first_command)
        .await
        .unwrap();
    let (first_ids, first_blocks) = match &first.effect {
        AuthorEditSettlementEffect::AuthoritativeApplied { ids, blocks, .. } => {
            (ids.clone(), blocks.clone())
        }
        effect => panic!("expected authoritative first effect, received {effect:?}"),
    };
    assert_eq!(first_blocks.len(), 1);
    assert_eq!(first_blocks[0].text, "Authoritative A!");
    assert_eq!(
        revision_member_block_id(
            &admin,
            &owner_user_id,
            &project_id,
            &chapter_id,
            &initial_revision_id,
        )
        .await,
        first_blocks[0].manuscript_block_id
    );
    assert_eq!(
        revision_member_block_id(
            &admin,
            &owner_user_id,
            &project_id,
            &chapter_id,
            &first_ids.revision_id,
        )
        .await,
        first_blocks[0].manuscript_block_id
    );
    assert_eq!(
        first.effect,
        AuthorEditSettlementEffect::AuthoritativeApplied {
            ids: first_ids.clone(),
            body: "Authoritative A!".to_owned(),
            blocks: first_blocks.clone(),
            author_action_sequence: 1,
            project_activity_position: 1,
        }
    );
    assert_eq!(
        read_settlement_owned_state(&admin, &owner_user_id, &project_id, &chapter_id).await,
        SettlementOwnedState {
            counters: ("1".to_owned(), "1".to_owned(), "1".to_owned()),
            head_revision_id: first_ids.revision_id.clone(),
            body: "Authoritative A!".to_owned(),
            latest_commit_sequence: "1".to_owned(),
            receipt_count: 1,
            revision_count: 2,
            commit_count: 1,
            author_action_count: 1,
            activity_count: 1,
            project_snapshot_count: 2,
            base_snapshot_count: 1,
            settlement_count: 1,
        }
    );

    admin
        .execute(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = 7,
                    authoritative_commit_sequence = 11,
                    project_activity_position = 13
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&owner_user_id, &project_id],
        )
        .await
        .unwrap();
    let mut existing_command = author_command(
        &scope,
        &editor_session_id,
        &Uuid::now_v7().to_string(),
        "sha256:counter-existing",
        "92",
    );
    set_unique_command_identity(&mut existing_command, &chapter_id);
    prepare_follow_up(
        &mut existing_command,
        &first_ids.revision_id,
        /*position*/ 16,
        "?",
    );
    issue_command_challenge(&store, &existing_command, "counter-existing-nonce").await;
    let existing = storyos_application::apply_author_edit(&store, &existing_command)
        .await
        .unwrap();
    let (existing_ids, existing_blocks) = match &existing.effect {
        AuthorEditSettlementEffect::AuthoritativeApplied { ids, blocks, .. } => {
            (ids.clone(), blocks.clone())
        }
        effect => panic!("expected authoritative existing effect, received {effect:?}"),
    };
    assert_eq!(
        existing_blocks[0].manuscript_block_id,
        first_blocks[0].manuscript_block_id
    );
    assert_eq!(
        existing.effect,
        AuthorEditSettlementEffect::AuthoritativeApplied {
            ids: existing_ids.clone(),
            body: "Authoritative A!?".to_owned(),
            blocks: existing_blocks,
            author_action_sequence: 8,
            project_activity_position: 14,
        }
    );
    assert_eq!(
        storyos_application::apply_author_edit(&store, &existing_command)
            .await
            .unwrap(),
        existing
    );
    assert_eq!(
        read_settlement_owned_state(&admin, &owner_user_id, &project_id, &chapter_id).await,
        SettlementOwnedState {
            counters: ("8".to_owned(), "12".to_owned(), "14".to_owned()),
            head_revision_id: existing_ids.revision_id.clone(),
            body: "Authoritative A!?".to_owned(),
            latest_commit_sequence: "12".to_owned(),
            receipt_count: 2,
            revision_count: 3,
            commit_count: 2,
            author_action_count: 2,
            activity_count: 2,
            project_snapshot_count: 3,
            base_snapshot_count: 1,
            settlement_count: 2,
        }
    );

    admin
        .execute(
            "UPDATE storyos.scope_counters
                SET authoritative_commit_sequence = $3::text::numeric
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&owner_user_id, &project_id, &MAX_COUNTER],
        )
        .await
        .unwrap();
    let before_limit =
        read_settlement_owned_state(&admin, &owner_user_id, &project_id, &chapter_id).await;
    let mut limit_command = author_command(
        &scope,
        &editor_session_id,
        &Uuid::now_v7().to_string(),
        "sha256:counter-limit",
        "93",
    );
    set_unique_command_identity(&mut limit_command, &chapter_id);
    prepare_follow_up(
        &mut limit_command,
        &existing_ids.revision_id,
        /*position*/ 17,
        ".",
    );
    issue_command_challenge(&store, &limit_command, "counter-limit-nonce").await;
    let limit_error = storyos_application::apply_author_edit(&store, &limit_command)
        .await
        .expect_err("the counter limit must reject settlement");
    assert!(matches!(limit_error, AuthorEditError::Unavailable(_)));
    assert_eq!(
        read_settlement_owned_state(&admin, &owner_user_id, &project_id, &chapter_id).await,
        before_limit
    );

    let retained = admin
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.author_command_admissions
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND author_command_admission_id = $3::text::uuid),
               (SELECT count(*) FROM storyos.author_command_admission_settlements
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND author_command_admission_id = $3::text::uuid),
               (SELECT outcome_kind FROM storyos.command_idempotency
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = 'applyAuthorEdit'
                   AND idempotency_key = $4::text::uuid),
               (SELECT result_reference::text FROM storyos.command_idempotency
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = 'applyAuthorEdit'
                   AND idempotency_key = $4::text::uuid),
               (SELECT consumed_at IS NOT NULL FROM storyos.project_command_challenges
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = 'applyAuthorEdit'
                   AND idempotency_key = $4::text::uuid)",
            &[
                &owner_user_id,
                &project_id,
                &limit_command.ids.author_command_admission_id,
                &limit_command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            retained.get::<_, i64>(0),
            retained.get::<_, i64>(1),
            retained.get::<_, String>(2),
            retained.get::<_, Option<String>>(3),
            retained.get::<_, bool>(4),
        ),
        (1, 0, "in_progress".to_owned(), None, true)
    );
}
