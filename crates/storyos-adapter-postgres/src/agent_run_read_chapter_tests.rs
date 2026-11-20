//! The Host Tool `read_chapter` (ADR 0095 section 1): the settled read of the head of a Chapter,
//! the request after it, each refusal, more than one read in one response, the bound, a change
//! between two reads, a full input after an expired reference, and the Project Scope.

use serde_json::{Value, json};
use storyos_application::{
    ChapterId, ConversationSelection, DeleteChapterInput, UpdateChapterInput,
};
use storyos_core::{CallValidationRefusal, ChapterReadRefusal, IntakeRefusal};

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_read_chapter::{
    boundary, call_ids, finish, no_public_call, outputs_record, project, range_text, read_text,
    sent, write_blocks,
};
use crate::shared_tests::agent_run_steering::{AuthorAction, AuthorAt, Moment, queued, settle};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::{admitted, head, rename};
use crate::shared_tests::command_sequence_structure::{
    DELETE_CHAPTER, UPDATE_CHAPTER, update_chapter,
};
use crate::shared_tests::command_sequence_support::issued;
use crate::shared_tests::command_sequence_support::stores;

fn settled(text: &str) -> Value {
    json!(["read_chapter", "settled", null, text, true])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn reads_settle_with_the_head_text_and_the_next_request_sends_each_as_a_manuscript_read() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let sentences = ["Rain <fell>.", "Wind & \"rose\".", "Night came."];
    let (scope, chapter_a, fog, _) = project(&store, &admin, "171", 0x1810, &sentences).await;
    let new = ConversationSelection::New;
    // Three reads in one response, the second of the same Chapter, and then one read of each
    // refused kind: Fog, which the author removes during the second request, 0, a number above
    // the count, and a value that is not an integer.
    let message = "SCRIPT:read_chapter 1,2,1;2,0,3,\"2\"";
    let run_id = admitted(&store, &scope, &chapter_a, "172", message, new).await;
    let fog_head = head(&admin, &fog).await.0;
    let input = DeleteChapterInput {
        chapter_id: ChapterId::new(fog.as_str()),
        expected_tree_revision: 4,
    };
    let removed = issued(&store, &scope, 0x1813, &DELETE_CHAPTER, input).await;
    let removed = AuthorAction::DeleteChapter(Box::new(removed));
    let author = AuthorAt::new(&store, Moment::DuringRequest(2), removed);
    settle(&store, &admin, &scope, &run_id, &author)
        .await
        .unwrap();

    let a_head = head(&admin, &chapter_a).await.0;
    let a_text = read_text(1, ("Chapter A", &chapter_a, &a_head), &sentences);
    let fog_text = read_text(2, ("Fog", &fog, &fog_head), &[""]);
    let not_found = ChapterReadRefusal::NotFound;
    let malformed = CallValidationRefusal::Intake(IntakeRefusal::Malformed);
    let refused = json!([
        "read_chapter",
        "refused",
        not_found.label(),
        not_found.result().text,
        true
    ]);
    // The Host executes the reads in call order. Each has no Destination Attempt and no Outbound
    // Disclosure Event, and each refused call gives its reason, and the Run continues.
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                settled(&a_text),
                settled(&fog_text),
                settled(&a_text),
                refused,
                refused,
                refused,
                [
                    "read_chapter",
                    "refused",
                    malformed.label(),
                    malformed.result_text(),
                    true
                ],
                finish(),
            ]
        ])
    );
    // The request after the reads sends each result as the output of its call, in the
    // `tool_result` boundary, and it continues the prior response.
    let outputs = sent(&admin, &run_id).await;
    let output = |index: usize, state: &str, text: &str| {
        let call_id = format!("call_read_chapter_{index}");
        json!([call_id, boundary(&call_id, state, text)])
    };
    let reads = [(0, &a_text), (1, &fog_text), (2, &a_text)].map(|(i, t)| output(i, "ok", t));
    assert_eq!(outputs[1], json!(["decision", 1, reads, false]));
    let rejected = output(6, "rejected", &malformed.result_text());
    assert_eq!(outputs[2][2][6], rejected);
    // The settled reads are `manuscript_read_result` items with the ToolCall identity, and the
    // refused reads stay `tool_result` items. Each has the trust `data_only`.
    let ids = call_ids(&admin, &run_id, Some("read_chapter")).await;
    let settled_at = |index: usize| index < 3;
    let sources: Vec<Value> = (0..ids.len())
        .map(|i| json!(["data_only", if settled_at(i) { "ok" } else { "rejected" }]))
        .collect();
    let items: Vec<Value> = (ids.iter().enumerate())
        .map(|(i, id)| match settled_at(i) {
            true => json!(["manuscript_read_result", id, "exact_required"]),
            false => json!(["tool_result", id, "exact_required"]),
        })
        .collect();
    assert_eq!(
        outputs_record(&admin, &run_id, 2).await,
        json!([sources, items])
    );
    // The public ToolCall list has no Host ToolCall.
    assert!(no_public_call(&store, &admin, &scope, &run_id, &ids[0]).await);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_read_gives_the_new_head_and_the_chapter_that_has_the_number_now() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, fog, session) = project(&store, &admin, "173", 0x1820, &["Rain."]).await;
    let first_head = head(&admin, &chapter_a).await.0;
    let new = ConversationSelection::New;
    let run_id = admitted(
        &store,
        &scope,
        &chapter_a,
        "174",
        "SCRIPT:read_chapter 1,2;1,2",
        new,
    );
    let run_id = run_id.await;
    // While the second request is out, the author edits Chapter A and moves Fog before it.
    let edit = (session.as_str(), 2, "1823");
    let edit = write_blocks(&store, &admin, &scope, edit, &chapter_a, &[" Snow."]).await;
    let input = UpdateChapterInput {
        chapter_id: ChapterId::new(fog.as_str()),
        title: "Fog".to_owned(),
        order: 1,
        expected_tree_revision: 4,
    };
    let moved = issued(&store, &scope, 0x1824, &UPDATE_CHAPTER, input).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(2),
        AuthorAction::Sequence(vec![
            AuthorAction::Edit(Box::new(edit)),
            AuthorAction::UpdateChapter(Box::new(moved)),
        ]),
    );
    settle(&store, &admin, &scope, &run_id, &author)
        .await
        .unwrap();

    let new_head = head(&admin, &chapter_a).await.0;
    let fog_head = head(&admin, &fog).await.0;
    assert_ne!(new_head, first_head);
    // The earlier results keep their text. The later read of the number 1 gives Fog, and the
    // later read of Chapter A gives the text and the revision of its new head.
    let a = |number, revision: &str, text: &str| {
        settled(&read_text(
            number,
            ("Chapter A", &chapter_a, revision),
            &[text],
        ))
    };
    let fog_at = |number| settled(&read_text(number, ("Fog", &fog, &fog_head), &[""]));
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                a(1, &first_head, "Rain."),
                fog_at(2),
                fog_at(1),
                a(2, &new_head, " Snow.Rain."),
                finish(),
            ]
        ])
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_chapter_above_the_item_limit_gives_its_whole_blocks_that_fit_and_a_range_the_rest() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The Chapter fits as the Working text, and its read with the header and the boundary does
    // not fit in the item limit.
    let (first, second) = (
        "Rain fell on the quay. ".repeat(700),
        "Wind rose. ".repeat(1_430),
    );
    let sentences = [first.as_str(), second.as_str()];
    let (scope, chapter_a, _, _) = project(&store, &admin, "175", 0x1830, &sentences).await;
    let new = ConversationSelection::New;
    let run_id = admitted(
        &store,
        &scope,
        &chapter_a,
        "176",
        "SCRIPT:read_chapter 1;1:2-null",
        new,
    )
    .await;
    complete(&store, &admin, &scope, &run_id).await;

    // The result has the first Block and one line that says where the Chapter continues, and its
    // output fits in the item limit. The second read gives the rest from block 2.
    let revision = head(&admin, &chapter_a).await.0;
    let a = ("Chapter A", chapter_a.as_str(), revision.as_str());
    let text = format!(
        "{}\n[The Chapter continues at block 2. This result does not have block 2 or the blocks \
         after it. Read them with first_block 2.]",
        range_text(1, a, (2, 1), &[&first])
    );
    let rest = range_text(1, a, (2, 2), &[&second]);
    let output = sent(&admin, &run_id).await[1][2][0][1].clone();
    let ids = call_ids(&admin, &run_id, Some("read_chapter")).await;
    assert_eq!(
        (
            calls(&admin, &run_id).await[1].as_array().unwrap()[..2].to_vec(),
            output.clone(),
            storyos_core::count_stored_text(output.as_str().unwrap()).character_count
                <= storyos_core::CONTEXT_ITEM_TOKEN_LIMIT,
            outputs_record(&admin, &run_id, 1).await[1].clone(),
        ),
        (
            vec![settled(&text), settled(&rest)],
            boundary("call_read_chapter_0", "ok", &text),
            true,
            json!([["manuscript_read_result", ids[0], "exact_required"]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_after_an_expired_reference_sends_each_read_output_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _, _) = project(&store, &admin, "177", 0x1840, &["Rain."]).await;
    let new = ConversationSelection::New;
    let message = "SCRIPT:read_chapter 1\nSCRIPT:step-reference-expires";
    let run_id = admitted(&store, &scope, &chapter_a, "178", message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let revision = head(&admin, &chapter_a).await.0;
    let text = read_text(1, ("Chapter A", &chapter_a, &revision), &["Rain."]);
    let read = json!([
        "call_read_chapter_0",
        boundary("call_read_chapter_0", "ok", &text)
    ]);
    let attempts = sent(&admin, &run_id).await;
    // The destination refuses the expired reference, and the full input sends the current input
    // and the read output. The read output is not a part of the current input.
    assert_eq!(
        (attempts[1].clone(), attempts[2].clone()),
        (
            json!(["rejected_create", 1, [read], false]),
            json!(["decision", 1, [read], true]),
        )
    );
    let ids = call_ids(&admin, &run_id, Some("read_chapter")).await;
    assert_eq!(
        outputs_record(&admin, &run_id, 1).await,
        json!([
            [["data_only", "ok"]],
            [["manuscript_read_result", ids[0], "exact_required"]]
        ])
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_same_number_reads_the_chapter_of_the_project_of_each_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut results = Vec::new();
    for (lead, suffix, title) in [("179", 0x1850, "Harbor"), ("17b", 0x1860, "Quay")] {
        let (scope, chapter_id, first) = queued(&store, lead).await;
        complete(&store, &admin, &scope, &first).await;
        let call = rename(&store, &scope, suffix, &chapter_id, title).await;
        update_chapter(&store, &call).await.unwrap();
        let prefix = format!("{:03x}", u16::from_str_radix(lead, 16).unwrap() + 1);
        let new = ConversationSelection::New;
        let message = "SCRIPT:read_chapter 1";
        let run_id = admitted(&store, &scope, &chapter_id, &prefix, message, new).await;
        complete(&store, &admin, &scope, &run_id).await;
        let revision = head(&admin, &chapter_id).await.0;
        let text = read_text(1, (title, &chapter_id, &revision), &[""]);
        results.push((calls(&admin, &run_id).await[1][0].clone(), settled(&text)));
    }

    // Each Run reads the Chapter of its own Project.
    assert_eq!(
        results.iter().map(|(got, _)| got).collect::<Vec<_>>(),
        results.iter().map(|(_, want)| want).collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_range_gives_its_blocks_and_a_range_outside_the_chapter_is_refused() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let blocks = ["One.", "Two.", "Three.", "Four."];
    let (scope, chapter_a, _, _) = project(&store, &admin, "2a1", 0x4b10, &blocks).await;
    // A range in the middle, a last Block above the count, and four invalid ranges.
    let message = "SCRIPT:read_chapter 1:2-3,1:3-9,1:0-2,1:-2-3,1:5-5,1:3-2";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_a, "2a2", message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let revision = head(&admin, &chapter_a).await.0;
    let a = ("Chapter A", chapter_a.as_str(), revision.as_str());
    let invalid = ChapterReadRefusal::BlockRangeInvalid { blocks: 4 };
    let refused = json!([
        "read_chapter",
        "refused",
        invalid.label(),
        invalid.result().text,
        true
    ]);
    // Each refused call gives the count of the Blocks, and the Run continues.
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                settled(&range_text(1, a, (4, 2), &["Two.", "Three."])),
                settled(&range_text(1, a, (4, 3), &["Three.", "Four."])),
                refused,
                refused,
                refused,
                refused,
                finish(),
            ]
        ])
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_range_names_the_blocks_of_the_new_head() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _, session) =
        project(&store, &admin, "2a3", 0x4b20, &["Rain.", "Wind."]).await;
    let first_head = head(&admin, &chapter_a).await.0;
    let new = ConversationSelection::New;
    let message = "SCRIPT:read_chapter 1:2-2;1:2-2";
    let run_id = admitted(&store, &scope, &chapter_a, "2a4", message, new).await;
    // While the second request is out, the author adds the Block "Snow." before "Rain.".
    let edit = (session.as_str(), 2, "4b23");
    let edit = write_blocks(&store, &admin, &scope, edit, &chapter_a, &["Snow.", ""]).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(2),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, &scope, &run_id, &author)
        .await
        .unwrap();

    // The same range names block 2 of each head: the second header has the new count and the
    // new revision.
    let new_head = head(&admin, &chapter_a).await.0;
    let at = |revision: &str, count, text: &str| {
        let a = ("Chapter A", chapter_a.as_str(), revision);
        settled(&range_text(1, a, (count, 2), &[text]))
    };
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                at(&first_head, 2, "Wind."),
                at(&new_head, 3, "Rain."),
                finish()
            ]
        ])
    );
}
