//! The Host Tool `search_manuscript` (ADR 0095 section 1): the settled search of the heads of the
//! live Chapters, a search with no match, more than one search in one response, the request after
//! it, the match limit and the bound, a change between two searches, a full input after an
//! expired reference, and the Project Scope.

use serde_json::{Value, json};
use storyos_application::{ConversationSelection, EditorSessionId};

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_read_chapter::{
    boundary, call_ids, finish, no_public_call, outputs_record, project, read_text, sent,
    write_blocks,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::{admitted, head, replace_text, select};
use crate::shared_tests::command_sequence_support::stores;

const NO_MATCH: &str = "No Block of the manuscript has this exact text, and this is not a failure.";

/// The header line of a result: the query, the count of matches, whether the search is
/// complete, the count of the Blocks with a match, and the count of the returned Blocks.
fn header(
    query: &str,
    (matches, complete): (usize, bool),
    (blocks, returned): (usize, usize),
) -> Value {
    json!({
        "blocks": blocks,
        "blocks_returned": returned,
        "complete": complete,
        "matches": matches,
        "query": query,
    })
}

/// The line of the Chapter `number` before its returned Blocks.
fn chapter_line(number: i64, title: &str, chapter_id: &str) -> Value {
    json!({"chapter": number, "chapter_id": chapter_id, "title": title})
}

/// The result of a search of `query` that has one match in the first Block `text` of Chapter 1.
fn one_match(query: &str, chapter_id: &str, text: &str) -> String {
    format!(
        "{}\n{}\n[block 1: 1 match]\n{text}",
        header(query, (1, true), (1, 1)),
        chapter_line(1, "Chapter A", chapter_id)
    )
}

fn no_match(query: &str) -> String {
    format!("{}\n{NO_MATCH}", header(query, (0, true), (0, 0)))
}

fn settled(text: &str) -> Value {
    json!(["search_manuscript", "settled", null, text, true])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn searches_settle_with_the_head_text_and_the_next_request_sends_each_as_a_search_result() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let a_blocks = ["Lin <ran>. Lin & \"Mei\".", "No one.", "Then Lin."];
    let (scope, chapter_a, fog, session) = project(&store, &admin, "191", 0x1a10, &a_blocks).await;
    // The author selects Fog and writes two Blocks into it.
    let writer = EditorSessionId::new(session.as_str());
    let call = select(&store, &admin, &scope, (&writer, 0x1a13), &fog, &chapter_a).await;
    store
        .set_current_chapter(&call.envelope, &call.input)
        .await
        .unwrap();
    let fog_blocks = ["Lin came.", "Fog."];
    let edit = (session.as_str(), 2, "1a14");
    let write = write_blocks(&store, &admin, &scope, edit, &fog, &fog_blocks).await;
    store.apply_author_edit(&write).await.unwrap();
    // Two searches and one read in one response. The second search has no match.
    let message =
        "SCRIPT:search_manuscript Lin\nSCRIPT:search_manuscript Wei\nSCRIPT:read_chapter 2";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &fog, "192", message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let lin = format!(
        "{}\n{}\n[block 1: 2 matches]\n{}\n[block 3: 1 match]\n{}\n{}\n[block 1: 1 match]\n{}",
        header("Lin", (4, true), (3, 3)),
        chapter_line(1, "Chapter A", &chapter_a),
        a_blocks[0],
        a_blocks[2],
        chapter_line(2, "Fog", &fog),
        fog_blocks[0]
    );
    let fog_head = head(&admin, &fog).await.0;
    let read = read_text(2, ("Fog", &fog, &fog_head), &fog_blocks);
    // The Host executes the calls in call order. Each has no Destination Attempt and no Outbound
    // Disclosure Event.
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                settled(&lin),
                settled(&no_match("Wei")),
                ["read_chapter", "settled", null, read, true],
                finish(),
            ]
        ])
    );
    // The request after the calls sends each result as the output of its call, in the
    // `tool_result` boundary, and it continues the prior response.
    let output = |index: usize, name: &str, text: &str| {
        let call_id = format!("call_{name}_{index}");
        json!([call_id, boundary(&call_id, "ok", text)])
    };
    let outputs = [
        output(0, "search_manuscript", &lin),
        output(1, "search_manuscript", &no_match("Wei")),
        output(2, "read_chapter", &read),
    ];
    assert_eq!(
        sent(&admin, &run_id).await[1],
        json!(["decision", 1, outputs, false])
    );
    // Each search result is a `manuscript_search_result` item with the ToolCall identity and the
    // trust `data_only`.
    let ids = call_ids(&admin, &run_id, None).await;
    assert_eq!(
        outputs_record(&admin, &run_id, 1).await,
        json!([
            [
                ["data_only", "ok"],
                ["data_only", "ok"],
                ["data_only", "ok"]
            ],
            [
                ["manuscript_search_result", ids[0], "exact_required"],
                ["manuscript_search_result", ids[1], "exact_required"],
                ["manuscript_read_result", ids[2], "exact_required"],
            ]
        ])
    );
    assert!(no_public_call(&store, &admin, &scope, &run_id, &ids[0]).await);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_search_finds_the_new_head_and_the_earlier_result_stays() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _, session) =
        project(&store, &admin, "193", 0x1a20, &["Lin ran."]).await;
    let message = "SCRIPT:search_manuscript Lin";
    let new = ConversationSelection::New;
    let first = admitted(&store, &scope, &chapter_a, "194", message, new.clone()).await;
    complete(&store, &admin, &scope, &first).await;
    let before = calls(&admin, &first).await;
    // An Author Edit removes the one match.
    let edit = (session.as_str(), 2, "1a23");
    let edit = replace_text(&store, &admin, &scope, edit, &chapter_a, (3, "Wei")).await;
    store.apply_author_edit(&edit).await.unwrap();
    let later = admitted(&store, &scope, &chapter_a, "195", message, new).await;
    complete(&store, &admin, &scope, &later).await;

    assert_eq!(
        (
            before.clone(),
            calls(&admin, &first).await,
            calls(&admin, &later).await
        ),
        (
            json!([
                "completed",
                [settled(&one_match("Lin", &chapter_a, "Lin ran.")), finish()]
            ]),
            before,
            json!(["completed", [settled(&no_match("Lin")), finish()]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_search_above_the_match_limit_gives_its_whole_blocks_that_fit_the_item() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The Chapter fits as the Working text. The search has 1 match in the first Block and 600 in
    // the second, so it stops at 500, and its result with both Blocks does not fit in the item.
    let first = format!("Lin waited. {}", "Rain fell on the quay. ".repeat(699));
    let second = format!("{}{}", "Lin ".repeat(600), "Wind rose. ".repeat(1_212));
    let blocks = [first.as_str(), second.as_str()];
    let (scope, chapter_a, _, _) = project(&store, &admin, "196", 0x1a30, &blocks).await;
    let message = "SCRIPT:search_manuscript Lin";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_a, "197", message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let text = format!(
        "{}\n{}\n[block 1: 1 match]\n{first}\n[This result does not have the other 1 Blocks with \
         a match. Search for a more exact term to find them.]",
        header("Lin", (500, false), (2, 1)),
        chapter_line(1, "Chapter A", &chapter_a)
    );
    let output = sent(&admin, &run_id).await[1][2][0][1].clone();
    let ids = call_ids(&admin, &run_id, None).await;
    assert_eq!(
        (
            calls(&admin, &run_id).await[1][0].clone(),
            output.clone(),
            storyos_core::count_stored_text(output.as_str().unwrap()).character_count
                <= storyos_core::CONTEXT_ITEM_TOKEN_LIMIT,
            outputs_record(&admin, &run_id, 1).await[1].clone(),
        ),
        (
            settled(&text),
            boundary("call_search_manuscript_0", "ok", &text),
            true,
            json!([["manuscript_search_result", ids[0], "exact_required"]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_after_an_expired_reference_sends_each_search_output_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _, _) = project(&store, &admin, "198", 0x1a40, &["Lin ran."]).await;
    let message = "SCRIPT:search_manuscript Lin\nSCRIPT:step-reference-expires";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_a, "199", message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let text = one_match("Lin", &chapter_a, "Lin ran.");
    let search = json!([
        "call_search_manuscript_0",
        boundary("call_search_manuscript_0", "ok", &text)
    ]);
    let attempts = sent(&admin, &run_id).await;
    // The destination refuses the expired reference, and the full input sends the current input
    // and the search output. The search output is not a part of the current input.
    assert_eq!(
        (attempts[1].clone(), attempts[2].clone()),
        (
            json!(["rejected_create", 1, [search], false]),
            json!(["decision", 1, [search], true]),
        )
    );
    let ids = call_ids(&admin, &run_id, None).await;
    assert_eq!(
        outputs_record(&admin, &run_id, 1).await,
        json!([
            [["data_only", "ok"]],
            [["manuscript_search_result", ids[0], "exact_required"]]
        ])
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_search_finds_only_the_text_of_the_project_of_its_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut results = Vec::new();
    for (lead, suffix, prefix, text) in [
        ("19a", 0x1a50, "19b", "Lin ran."),
        ("19c", 0x1a60, "19d", "Lin met Mei."),
    ] {
        let (scope, chapter_a, _, _) = project(&store, &admin, lead, suffix, &[text]).await;
        let message = "SCRIPT:search_manuscript Lin\nSCRIPT:search_manuscript Mei";
        let new = ConversationSelection::New;
        let run_id = admitted(&store, &scope, &chapter_a, prefix, message, new).await;
        complete(&store, &admin, &scope, &run_id).await;
        let mei = match text.contains("Mei") {
            true => one_match("Mei", &chapter_a, text),
            false => no_match("Mei"),
        };
        let want = json!([
            "completed",
            [
                settled(&one_match("Lin", &chapter_a, text)),
                settled(&mei),
                finish()
            ]
        ]);
        results.push((calls(&admin, &run_id).await, want));
    }

    // Each Run finds the text of its own Project only, and a term that only the other Project
    // has gives no match.
    assert_eq!(
        results.iter().map(|(got, _)| got).collect::<Vec<_>>(),
        results.iter().map(|(_, want)| want).collect::<Vec<_>>()
    );
}
