//! The `chapter_list` item of each RunStep of a Run, at each request of the Run (ADR 0095
//! section 1, ADR 0085).

use serde_json::{Value, json};
use storyos_application::{
    ChapterId, ConversationSelection, CreateChapterInput, IssueProjectCommandChallenge,
    ProjectScope, UpdateChapterInput, VerifiedExportArchive, claim_next_archive_export,
    complete_archive_export, get_verified_export_archive, issue_project_command_challenge,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, CreateChapterPlacement,
    DeploymentDestination, hex_sha256,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_run_in, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle, steering};
use crate::command_sequence::tests::{
    CREATE_CHAPTER, CommandCall, UPDATE_CHAPTER, issued, update_chapter,
};
use crate::project_archive_build::tests::{export_command, zip_store_files};

/// The list of the fixture Project: one Volume with its Chapter, the Current Chapter of the Run.
const FIXTURE_LIST: &str = "{\"volume\":\"Volume A\"}\n[1,0,\"Chapter A\",\"current\"]";

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, the
/// text of each `chapter_list` item that the fake destination received, and the replaced input
/// label.
async fn sent(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position,
                      jsonb_path_query_array(wire,
                        '$.context[*] ? (@.source_class == \"chapter_list\").content'),
                      wire->'replaces_earlier_input')
                      ORDER BY model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// For each decision Context Assembly of the Run in RunStep order: its RunStep and its selected
/// `chapter_list` items.
async fn recorded(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(decision_position,
                      (SELECT jsonb_agg(jsonb_build_array(item->>'projection_mode',
                                                          item->>'source_version',
                                                          item->>'content'))
                         FROM jsonb_array_elements(payload->'selected') AS item
                        WHERE item->>'source_class' = 'chapter_list'))
                      ORDER BY decision_position)::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The recorded `chapter_list` item with `text`.
fn item(text: &str) -> Value {
    json!([
        "exact_required",
        format!("sha256:{}", hex_sha256(text.as_bytes())),
        text
    ])
}

/// The Conversation of the Run that `prefix` admitted.
fn conversation_of(prefix: &str) -> ConversationSelection {
    ConversationSelection::Existing {
        conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
    }
}

/// Admits one Run of the tool loop profile with `message` in `conversation` of the Project.
async fn admitted(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    message: &str,
    conversation: ConversationSelection,
) -> String {
    let destination = DeploymentDestination::HostFakeToolLoop;
    admit_run_in(
        store,
        scope,
        chapter_id,
        prefix,
        destination,
        message,
        conversation,
        None,
    )
    .await
}

/// The rename of the fixture Chapter to `title`, with a fresh challenge `suffix`.
async fn rename(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    chapter_id: &str,
    title: &str,
) -> CommandCall<UpdateChapterInput> {
    let input = UpdateChapterInput {
        chapter_id: ChapterId::new(chapter_id),
        title: title.to_owned(),
        order: 1,
        expected_tree_revision: 3,
    };
    issued(store, scope, suffix, &UPDATE_CHAPTER, input).await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_step_records_the_list_and_an_unchanged_incremental_request_sends_none() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued(&store, "0e1").await;
    complete(&store, &admin, &scope, &run_id).await;

    // The request after the `finish` output continues with an unchanged input, so it sends no
    // list, but its Context Assembly records the same item.
    assert_eq!(
        (recorded(&admin, &run_id).await, sent(&admin, &run_id).await),
        (
            json!([[0, [item(FIXTURE_LIST)]], [1, [item(FIXTURE_LIST)]]]),
            json!([
                ["decision", 0, [FIXTURE_LIST], null],
                ["decision", 1, [], null]
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_a_compaction_and_a_replaced_input_send_the_list_and_a_summary_sends_none() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The destination lost the closing reference of the Run before the third Run, so the third
    // Run sends its full input again.
    let (scope, chapter_id, first) = queued(&store, "0e2").await;
    complete(&store, &admin, &scope, &first).await;
    let message = "Help with this passage. SCRIPT:reference-expires";
    let expiring = admitted(
        &store,
        &scope,
        &chapter_id,
        "0e3",
        message,
        conversation_of("0e2"),
    );
    let expiring = expiring.await;
    complete(&store, &admin, &scope, &expiring).await;
    let expired = admitted(
        &store,
        &scope,
        &chapter_id,
        "0e4",
        "Help.",
        conversation_of("0e2"),
    );
    let expired = expired.await;
    complete(&store, &admin, &scope, &expired).await;
    let compacting = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let (scope, chapter_id, first) = queued(&store, "0e5").await;
    complete(&store, &admin, &scope, &first).await;
    let new = ConversationSelection::New;
    let compacted = admitted(&store, &scope, &chapter_id, "0e6", &compacting, new).await;
    complete(&store, &admin, &scope, &compacted).await;
    let (scope, _, steered) = queued(&store, "0e7").await;
    let input = steering(&store, &scope, 0x0f00, &steered, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &steered, &author)
        .await
        .unwrap();

    assert_eq!(
        (
            sent(&admin, &expired).await,
            sent(&admin, &compacted).await,
            sent(&admin, &steered).await,
        ),
        (
            json!([
                ["rejected_create", 0, [FIXTURE_LIST], null],
                ["decision", 0, [FIXTURE_LIST], null],
                ["decision", 1, [], null],
            ]),
            json!([
                ["decision", 0, [FIXTURE_LIST], null],
                ["compaction", 1, [], null],
                ["decision", 1, [FIXTURE_LIST], null],
            ]),
            // The steering input replaces the input of RunStep 1.
            json!([
                ["decision", 0, [FIXTURE_LIST], null],
                ["decision", 1, [FIXTURE_LIST], true],
                ["decision", 2, [], null]
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_rename_and_a_new_chapter_between_two_run_steps_send_the_new_list_as_a_replaced_input() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, renamed) = queued(&store, "0e8").await;
    let call = rename(&store, &scope, 0x0f10, &chapter_id, "Harbor").await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::UpdateChapter(Box::new(call)),
    );
    settle(&store, &admin, &scope, &renamed, &author)
        .await
        .unwrap();
    let (scope, chapter_id, extended) = queued(&store, "0e9").await;
    let volume_id: String = admin
        .query_one(
            "SELECT parent_volume_id::text FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let input = CreateChapterInput {
        volume_id,
        title: "Fog".to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision: 3,
    };
    let call = issued(&store, &scope, 0x0f11, &CREATE_CHAPTER, input).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::CreateChapter(Box::new(call)),
    );
    settle(&store, &admin, &scope, &extended, &author)
        .await
        .unwrap();

    // The next RunStep assembles the list from the manuscript at that time. Its input digest is
    // different, so its request sends the current input again with the replaced input label.
    let with_new = format!("{FIXTURE_LIST}\n[2,0,\"Fog\"]");
    assert_eq!(
        (sent(&admin, &renamed).await, sent(&admin, &extended).await),
        (
            json!([
                ["decision", 0, [FIXTURE_LIST], null],
                [
                    "decision",
                    1,
                    ["{\"volume\":\"Volume A\"}\n[1,0,\"Harbor\",\"current\"]"],
                    true
                ]
            ]),
            json!([
                ["decision", 0, [FIXTURE_LIST], null],
                ["decision", 1, [with_new], true]
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_list_of_a_run_has_only_the_chapters_of_its_own_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (first_scope, _, first) = queued(&store, "0ea").await;
    complete(&store, &admin, &first_scope, &first).await;
    // The admission assembles the first RunStep, so the Chapter of the second Project gets its
    // own title before the Run of the second Project.
    let (second_scope, chapter_id, earlier) = queued(&store, "0eb").await;
    complete(&store, &admin, &second_scope, &earlier).await;
    let call = rename(&store, &second_scope, 0x0f12, &chapter_id, "Fog").await;
    update_chapter(&store, &call).await.unwrap();
    let new = ConversationSelection::New;
    let second = admitted(&store, &second_scope, &chapter_id, "0ed", "Help.", new).await;
    complete(&store, &admin, &second_scope, &second).await;

    assert_eq!(
        (
            recorded(&admin, &first).await[0][1].clone(),
            recorded(&admin, &second).await[0][1].clone(),
        ),
        (
            json!([item(FIXTURE_LIST)]),
            json!([item("{\"volume\":\"Volume A\"}\n[1,0,\"Fog\",\"current\"]")]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_archive_of_a_project_whose_run_has_the_list_builds_and_packs_its_records() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued(&store, "0ec").await;
    complete(&store, &admin, &scope, &run_id).await;
    let (export, input) = export_command(&scope, "0e");
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: "nonce-export-0e".to_owned(),
            nonce_digest: export.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store.export_project_archive(&export, &input).await.unwrap();
    let claimed = claim_next_archive_export(&store).await.unwrap().unwrap();
    complete_archive_export(&store, &claimed).await.unwrap();
    let VerifiedExportArchive::Ready(zip) =
        get_verified_export_archive(&store, &scope, &input.export_id)
            .await
            .unwrap()
    else {
        panic!("the Project Export must return verified ZIP bytes");
    };
    let files = zip_store_files(&zip);
    let rows = &files
        .iter()
        .find(|(name, _)| *name == "canonical/context_assembly_manifests.json")
        .unwrap()
        .1;
    let packed: Vec<Value> = serde_json::from_slice(rows).unwrap();
    let lists: Vec<Value> = packed
        .iter()
        .filter(|row| row["run_id"] == json!(run_id) && row["manifest_role"] == "decision")
        .flat_map(|row| row["payload"]["selected"].as_array().unwrap().clone())
        .filter(|item| item["source_class"] == "chapter_list")
        .map(|item| item["content"].clone())
        .collect();

    assert_eq!(lists, [FIXTURE_LIST, FIXTURE_LIST]);
}
