//! The `skill_instruction` items of a Run whose author selected Skills, and the `skill_catalog`
//! item of a Run with a pinned catalog, at each request of the Run (ADR 0089 sections 3 and 5,
//! ADR 0085, ADR 0093).

use std::sync::Arc;

use serde_json::json;
use storyos_application::ConversationSelection;
use storyos_core::{ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX};

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_finish_rule::conversation_of;
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, queued, settle, steering,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::admit_selected_run;
use crate::shared_tests::skill_instruction_context::{
    catalog, of_conversation, pinned, recorded, requirements, selected, selected_run, sent, texts,
};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_step_selects_the_catalog_and_a_later_run_that_continues_sends_no_skill_md() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (packages, skills, _) = selected();
    let (scope, chapter_id, run_id) = selected_run(
        &store,
        &admin,
        ["441", "442"],
        "Help with this passage.",
        ConversationSelection::New,
    )
    .await;
    complete(&store, &admin, &scope, &run_id).await;
    let later = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "443",
        "Help again.",
        conversation_of("442"),
        (skills, Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &later).await;

    // The first request sends one section for each Skill in name order, then the catalog. The
    // request after the `finish` output continues with an unchanged input, so it sends no Skill
    // text and no catalog. The later Run records the Skills of the Conversation, and its first
    // request continues the prior response, which holds their text (ADR 0089 section 5).
    let (catalog, pinned) = (catalog(&packages), pinned(&packages));
    assert_eq!(
        (
            sent(&admin, &run_id).await,
            requirements(&admin, &run_id).await,
            sent(&admin, &later).await,
            requirements(&admin, &later).await,
        ),
        (
            json!([
                ["decision", 0, texts(&packages), [catalog]],
                ["decision", 1, [], []]
            ]),
            json!([
                [0, recorded(&packages), pinned],
                [1, recorded(&packages), pinned]
            ]),
            json!([["decision", 0, [], [catalog]], ["decision", 1, [], []]]),
            json!([
                [0, of_conversation(&packages), pinned],
                [1, of_conversation(&packages), pinned]
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_sends_the_same_skill_and_catalog_text_and_a_summary_request_sends_none() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (packages, skills, selections) = selected();
    let (text, catalog) = (texts(&packages), [catalog(&packages)]);
    // The destination lost the closing reference of the Run before the selected Run, so the
    // selected Run sends its full input again.
    let (scope, chapter_id, first) = queued(&store, "444").await;
    complete(&store, &admin, &scope, &first).await;
    let expiring = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "445",
        "Help with this passage. SCRIPT:reference-expires",
        conversation_of("444"),
        (Arc::clone(&skills), Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &expiring).await;
    let expired = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "446",
        "Help again.",
        conversation_of("444"),
        (skills, selections),
    )
    .await;
    complete(&store, &admin, &scope, &expired).await;
    let compacting = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let new = ConversationSelection::New;
    let (scope, _, compacted) =
        selected_run(&store, &admin, ["447", "448"], &compacting, new.clone()).await;
    complete(&store, &admin, &scope, &compacted).await;
    let (scope, _, steered) =
        selected_run(&store, &admin, ["449", "44a"], "Help again.", new).await;
    let input = steering(&store, &scope, 0x4500, &steered, "Keep the voice.", None).await;
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
            requirements(&admin, &steered).await,
        ),
        (
            json!([
                ["rejected_create", 0, text, catalog],
                ["decision", 0, text, catalog],
                ["decision", 1, [], []],
            ]),
            json!([
                ["decision", 0, text, catalog],
                ["compaction", 1, [], []],
                ["decision", 1, text, catalog],
            ]),
            // The steering input replaces the input of RunStep 1, and the requirement stays.
            json!([
                ["decision", 0, text, catalog],
                ["decision", 1, text, catalog],
                ["decision", 2, [], []]
            ]),
            json!([
                [0, recorded(&packages), pinned(&packages)],
                [1, recorded(&packages), pinned(&packages)],
                [2, recorded(&packages), pinned(&packages)],
            ]),
        )
    );
}

#[path = "conversation_skill_context_tests.rs"]
mod conversation;
