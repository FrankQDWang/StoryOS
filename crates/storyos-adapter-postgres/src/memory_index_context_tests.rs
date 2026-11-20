//! The pin of the Memory revision of a Run and its `memory_index` item at each request of the
//! Run (ADR 0094 section 4, ADR 0085).

use serde_json::{Value, json};
use storyos_application::{ChapterId, ConversationSelection, ProjectScope, UpdateChapterInput};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, hex_sha256,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_finish_rule::conversation_of;
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, queued, settle, steering,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::admitted;
use crate::shared_tests::command_sequence_structure::UPDATE_CHAPTER;
use crate::shared_tests::command_sequence_support::issued;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::write_versions;
use crate::shared_tests::model_gateway::sent_context;

/// The Index of the two entries that `seeded` writes at the Memory revision 2.
const INDEX: &str = "{\"type\":\"foreshadowing\"}\n\
    {\"description\":\"铃声。\",\"id\":2,\"title\":\"铃\"}\n{\"type\":\"character\"}\n\
    {\"description\":\"沉默的船夫。\",\"id\":1,\"title\":\"林舟\"}";

/// A Project of `prefix` whose first Run is complete, with two current entries at the Memory
/// revision 2.
async fn seeded(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) = queued(store, prefix).await;
    complete(store, admin, &scope, &first).await;
    let versions = [
        (1, 1, 1, "current", "character", "林舟", "沉默的船夫。"),
        (2, 1, 2, "current", "foreshadowing", "铃", "铃声。"),
    ];
    write_versions(admin, &scope, &versions).await;
    (scope, chapter_id)
}

/// For each decision RunStep of the Run in order: its position, the pinned Memory revision of
/// its Operation Requirement, and its selected `memory_index` items.
async fn recorded(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(manifest.decision_position,
                      requirement.payload #> '{operation_requirement,memory_revision}',
                      (SELECT COALESCE(jsonb_agg(jsonb_build_array(item->>'projection_mode',
                                                                   item->>'source_version',
                                                                   item->>'content')), '[]')
                         FROM jsonb_array_elements(manifest.payload->'selected') AS item
                        WHERE item->>'source_class' = 'memory_index'))
                      ORDER BY manifest.decision_position)::text
               FROM storyos.context_assembly_manifests AS manifest
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.run_id, requirement.decision_position) =
                    (manifest.run_id, manifest.decision_position)
              WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'",
        &[&run_id],
    )
    .await
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, the
/// text of each `memory_index` item that the fake destination received, and the replaced input
/// label.
async fn sent(admin: &Client, run_id: &str) -> Value {
    sent_context(admin, run_id, &["memory_index"], true).await
}

/// The recorded `memory_index` item with `text`.
fn item(text: &str) -> Value {
    json!([
        "exact_required",
        format!("sha256:{}", hex_sha256(text.as_bytes())),
        text
    ])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_keeps_its_pinned_index_and_a_later_run_gets_the_new_index() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let project = seeded(&store, &admin, "c54").await;
    let pinned = admitted(
        &store,
        &project.0,
        &project.1,
        "c55",
        "Help.",
        conversation_of("c54"),
    )
    .await;
    // A later transaction deletes the entry 2 and writes the entry 3 before the second RunStep.
    let changes = [
        (2, 2, 3, "deleted", "foreshadowing", "铃", "铃声。"),
        (3, 1, 4, "current", "character", "阿青", "林舟的妹妹。"),
    ];
    write_versions(&admin, &project.0, &changes).await;
    complete(&store, &admin, &project.0, &pinned).await;
    let later = admitted(
        &store,
        &project.0,
        &project.1,
        "c56",
        "Help.",
        conversation_of("c54"),
    )
    .await;
    complete(&store, &admin, &project.0, &later).await;
    let new = "{\"type\":\"character\"}\n{\"description\":\"沉默的船夫。\",\"id\":1,\"title\":\"林舟\"}\n\
               {\"description\":\"林舟的妹妹。\",\"id\":3,\"title\":\"阿青\"}";

    // The request after the `finish` output continues with an unchanged input, so it sends no
    // Index, but its Context Assembly records the same item.
    assert_eq!(
        (
            recorded(&admin, &pinned).await,
            sent(&admin, &pinned).await,
            recorded(&admin, &later).await,
        ),
        (
            json!([[0, 2, [item(INDEX)]], [1, 2, [item(INDEX)]]]),
            json!([["decision", 0, [INDEX], null], ["decision", 1, [], null]]),
            json!([[0, 4, [item(new)]], [1, 4, [item(new)]]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_a_compaction_a_steering_input_and_a_chapter_edit_send_the_same_index() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The destination lost the closing reference of the Run before the second Run, so the second
    // Run sends its full input again.
    let project = seeded(&store, &admin, "c57").await;
    let message = "Help with this passage. SCRIPT:reference-expires";
    let expiring = admitted(
        &store,
        &project.0,
        &project.1,
        "c58",
        message,
        conversation_of("c57"),
    )
    .await;
    complete(&store, &admin, &project.0, &expiring).await;
    let expired = admitted(
        &store,
        &project.0,
        &project.1,
        "c59",
        "Help.",
        conversation_of("c57"),
    )
    .await;
    complete(&store, &admin, &project.0, &expired).await;
    let project = seeded(&store, &admin, "c5a").await;
    let compacting = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let new = ConversationSelection::New;
    let compacted = admitted(&store, &project.0, &project.1, "c5b", &compacting, new).await;
    complete(&store, &admin, &project.0, &compacted).await;
    let steered = admitted(
        &store,
        &project.0,
        &project.1,
        "c5c",
        "Help.",
        ConversationSelection::New,
    )
    .await;
    let input = steering(
        &store,
        &project.0,
        0xcf0a,
        &steered,
        "Keep the voice.",
        None,
    )
    .await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &project.0, &steered, &author)
        .await
        .unwrap();
    let project = seeded(&store, &admin, "c5d").await;
    let renamed = admitted(
        &store,
        &project.0,
        &project.1,
        "c5e",
        "Help.",
        ConversationSelection::New,
    )
    .await;
    let rename = UpdateChapterInput {
        chapter_id: ChapterId::new(&project.1),
        title: "Harbor".to_owned(),
        order: 1,
        expected_tree_revision: 3,
    };
    let call = issued(&store, &project.0, 0xcf0b, &UPDATE_CHAPTER, rename).await;
    let action = AuthorAction::UpdateChapter(Box::new(call));
    let author = AuthorAt::new(&store, Moment::DuringRequest(1), action);
    settle(&store, &admin, &project.0, &renamed, &author)
        .await
        .unwrap();

    assert_eq!(
        (
            sent(&admin, &expired).await,
            sent(&admin, &compacted).await,
            sent(&admin, &steered).await,
            sent(&admin, &renamed).await,
        ),
        (
            json!([
                ["rejected_create", 0, [INDEX], null],
                ["decision", 0, [INDEX], null],
                ["decision", 1, [], null],
            ]),
            json!([
                ["decision", 0, [INDEX], null],
                ["compaction", 1, [], null],
                ["decision", 1, [INDEX], null],
            ]),
            // The steering input replaces the input of RunStep 1.
            json!([
                ["decision", 0, [INDEX], null],
                ["decision", 1, [INDEX], true],
                ["decision", 2, [], null]
            ]),
            // The rename changes the Chapter list, so RunStep 1 sends the current input again.
            json!([
                ["decision", 0, [INDEX], null],
                ["decision", 1, [INDEX], true]
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_without_use_or_without_a_current_entry_has_no_index() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The first Run of `seeded` starts before each entry, so it has no pin.
    let (scope, chapter_id, first) = queued(&store, "c5f").await;
    complete(&store, &admin, &scope, &first).await;
    let versions = [(1, 1, 1, "current", "character", "林舟", "沉默的船夫。")];
    write_versions(&admin, &scope, &versions).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.conversation_memory_settings SET is_current = FALSE
              WHERE project_id = '{project}'::uuid;
             INSERT INTO storyos.conversation_memory_settings
             SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
               (to_jsonb(settings) - 'is_current') || jsonb_build_object(
                 'memory_settings_revision', gen_random_uuid(), 'use_enabled', false))).*
               FROM storyos.conversation_memory_settings AS settings
              WHERE project_id = '{project}'::uuid",
            project = scope.project_id.as_ref()
        ))
        .await
        .unwrap();
    let project = (scope.clone(), chapter_id);
    let unused = admitted(
        &store,
        &project.0,
        &project.1,
        "cf1",
        "Help.",
        conversation_of("c5f"),
    )
    .await;
    complete(&store, &admin, &scope, &unused).await;
    // Each entry of the second Project is deleted, so the pin has no current entry.
    let (deleted, chapter_id, earlier) = queued(&store, "cf2").await;
    complete(&store, &admin, &deleted, &earlier).await;
    let versions = [
        (1, 1, 1, "current", "other", "渡口", "旧渡口。"),
        (1, 2, 2, "deleted", "other", "渡口", "旧渡口。"),
    ];
    write_versions(&admin, &deleted, &versions).await;
    let project = (deleted.clone(), chapter_id);
    let empty = admitted(
        &store,
        &project.0,
        &project.1,
        "cf3",
        "Help.",
        ConversationSelection::New,
    )
    .await;
    complete(&store, &admin, &deleted, &empty).await;

    assert_eq!(
        (
            recorded(&admin, &first).await,
            recorded(&admin, &unused).await,
            sent(&admin, &unused).await,
            recorded(&admin, &empty).await,
        ),
        (
            json!([[0, null, []], [1, null, []]]),
            json!([[0, null, []], [1, null, []]]),
            json!([["decision", 0, [], null], ["decision", 1, [], null]]),
            json!([[0, 2, []], [1, 2, []]]),
        )
    );
}
