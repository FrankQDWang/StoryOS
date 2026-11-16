//! The Host Tool `read_chapter` (ADR 0095 section 1): the settled read of the head of a Chapter,
//! the request after it, each refusal, more than one read in one response, the bound, a change
//! between two reads, a full input after an expired reference, and the Project Scope.

use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ApplyAuthorEditCommand,
    ChallengeRateClass, ChapterId, ConversationSelection, CreateChapterInput, DeleteChapterInput,
    EditorSessionStore, ProjectScope, UpdateChapterInput,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, CallValidationRefusal, ChapterReadRefusal,
    CreateChapterPlacement, IntakeRefusal, SelectionSnapshot, TransitionOutcome,
    UTF16_COORDINATE_PROFILE,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_load_skill::tests::calls;
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle};
use crate::command_sequence::tests::{
    CREATE_CHAPTER, DELETE_CHAPTER, UPDATE_CHAPTER, create_chapter, issued, update_chapter,
};
use crate::create_agent_run::context::chapter_list_tests::{admitted, head, rename};
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command};
use crate::set_current_chapter_authority_tests::open_session_request;

/// The expected result of a read of the Chapter `number`: the header line and each Block.
fn read_text(
    number: i64,
    (title, chapter_id, revision_id): (&str, &str, &str),
    blocks: &[&str],
) -> String {
    let header = json!({
        "blocks": blocks.len(),
        "chapter": number,
        "chapter_id": chapter_id,
        "last_block": blocks.len(),
        "revision_id": revision_id,
        "title": title,
    });
    let blocks: String = blocks
        .iter()
        .enumerate()
        .map(|(index, text)| format!("\n[block {}]\n{text}", index + 1))
        .collect();
    format!("{header}{blocks}")
}

fn settled(text: &str) -> Value {
    json!(["read_chapter", "settled", null, text, true])
}

fn finish() -> Value {
    json!(["finish", "settled", null, "", true])
}

/// The output of the call `call_id` in its `tool_result` boundary with the state `state`.
fn boundary(call_id: &str, state: &str, text: &str) -> Value {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    json!(format!(
        "<tool_result source=\"{call_id}\" state=\"{state}\">{escaped}</tool_result>"
    ))
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, each
/// Tool output that it sent, and whether it sent the current input.
async fn sent(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position,
                      COALESCE((SELECT jsonb_agg(jsonb_build_array(output ->> 'call_id',
                                                                   output ->> 'output'))
                                  FROM jsonb_array_elements(wire -> 'tool_outputs') AS output),
                               '[]'::jsonb), wire ? 'context')
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

/// The trust and state of each `tool_results` source, and the class, source version, and
/// projection mode of each function output item, of the decision Context Assembly at `position`.
async fn outputs_record(admin: &Client, run_id: &str, position: i32) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(
                      (SELECT jsonb_agg(jsonb_build_array(source ->> 'trust', source ->> 'state'))
                         FROM jsonb_array_elements(payload #> '{operation_requirement,tool_results}')
                              AS source),
                      (SELECT jsonb_agg(jsonb_build_array(item ->> 'source_class',
                                                          item ->> 'source_version',
                                                          item ->> 'projection_mode'))
                         FROM jsonb_array_elements(payload -> 'selected') AS item
                        WHERE item ->> 'source_class' IN ('tool_result', 'manuscript_read_result')))::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'
                AND decision_position = $2::text::numeric",
            &[&run_id, &position.to_string()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The identity of each `read_chapter` call of the Run, in call order.
async fn read_ids(admin: &Client, run_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT tool_call_id::text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'read_chapter'
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

/// The Author Edit of the Editor Session `session` that writes `sentences` into the empty first
/// Block of `chapter_id` and splits them into one Block each.
async fn write_blocks(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    (session, sequence, suffix): (&str, u64, &str),
    chapter_id: &str,
    sentences: &[&str],
) -> ApplyAuthorEditCommand {
    let (revision, first) = head(admin, chapter_id).await;
    let mut primitives = vec![AuthorEditPrimitive::ReplaceBlockSelection {
        manuscript_block_id: first.clone(),
        from: 0,
        to: 0,
        text: sentences.concat(),
    }];
    let mut block = first;
    for sentence in &sentences[..sentences.len() - 1] {
        let new_block = uuid::Uuid::now_v7().to_string();
        primitives.push(AuthorEditPrimitive::SplitBlock {
            manuscript_block_id: block,
            offset: u32::try_from(sentence.encode_utf16().count()).unwrap(),
            new_manuscript_block_id: new_block.clone(),
        });
        block = new_block;
    }
    let edit = NamedEdit {
        editor_session_id: session,
        chapter_id,
        expected_revision_id: &revision,
        suffix,
        local_intent_sequence: sequence,
        text: "",
        proposal_target: None,
    };
    adjusted_edit_command(store, scope, edit, |command| {
        // The Author Edit class keeps the Shared Challenges of the Project for the Runs.
        command.challenge_binding.challenge_rate_policy_revision =
            ChallengeRateClass::AuthorEdit.policy_revision().to_owned();
        command.author_edit_units = vec![AuthorEditUnit {
            normalized_primitives: primitives,
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: 0,
                to: 0,
            },
        }];
    })
    .await
}

/// A new Project whose Chapter A has the Blocks `sentences`, a second empty Chapter `Fog` at its
/// end, and an open Editor Session. `lead` names the Project. `suffix` is the challenge of the
/// new Chapter, `suffix + 1` the Editor Session, and `suffix + 2` the Author Edit.
async fn project(
    store: &PostgresProjectReader,
    admin: &Client,
    lead: &str,
    suffix: u16,
    sentences: &[&str],
) -> (ProjectScope, String, String, String) {
    let (scope, chapter_a, first) = queued(store, lead).await;
    complete(store, admin, &scope, &first).await;
    let volume_id: String = admin
        .query_one(
            "SELECT parent_volume_id::text FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_a],
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
    let call = issued(store, &scope, suffix, &CREATE_CHAPTER, input).await;
    let fog = match create_chapter(store, &call).await.unwrap().outcome {
        TransitionOutcome::Applied(applied) => applied.effect.chapter_id,
        other => panic!("{other:?}"),
    };
    let request = open_session_request(store, &scope, &format!("{:04x}", suffix + 1)).await;
    store.create_editor_session(&request).await.unwrap();
    let session = request.editor_session_id.as_ref().to_owned();
    let edit_suffix = format!("{:04x}", suffix + 2);
    let edit = (session.as_str(), 1, edit_suffix.as_str());
    let write = write_blocks(store, admin, &scope, edit, &chapter_a, sentences).await;
    store.apply_author_edit(&write).await.unwrap();
    (scope, chapter_a, fog, session)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn reads_settle_with_the_head_text_and_the_next_request_sends_each_as_a_manuscript_read() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let ids = read_ids(&admin, &run_id).await;
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
    let attempt_id: String = admin
        .query_one(
            "SELECT model_attempt_id::text FROM storyos.tool_calls
              WHERE tool_call_id = $1::text::uuid",
            &[&ids[0]],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let public = store
        .read_agent_run(
            &scope,
            &run_id,
            &AgentRunReadSelection::ModelAttempt(attempt_id),
        )
        .await
        .unwrap()
        .unwrap()
        .decision;
    assert!(matches!(
        public,
        AgentRunDecisionInspect::ToolRequest { ref calls, .. } if calls.is_empty()
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_read_gives_the_new_head_and_the_chapter_that_has_the_number_now() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
async fn a_chapter_above_the_item_limit_gives_its_whole_blocks_that_fit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
        "SCRIPT:read_chapter 1",
        new,
    )
    .await;
    complete(&store, &admin, &scope, &run_id).await;

    // The result has the first Block and one line that says where the Chapter continues, and its
    // output fits in the item limit.
    let revision = head(&admin, &chapter_a).await.0;
    let header = json!({
        "blocks": 2,
        "chapter": 1,
        "chapter_id": chapter_a,
        "last_block": 1,
        "revision_id": revision,
        "title": "Chapter A",
    });
    let text = format!(
        "{header}\n[block 1]\n{first}\n[The Chapter continues at block 2. This result does not \
         have block 2 or the blocks after it.]"
    );
    let output = sent(&admin, &run_id).await[1][2][0][1].clone();
    let ids = read_ids(&admin, &run_id).await;
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
            boundary("call_read_chapter_0", "ok", &text),
            true,
            json!([["manuscript_read_result", ids[0], "exact_required"]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_after_an_expired_reference_sends_each_read_output_again() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let ids = read_ids(&admin, &run_id).await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
