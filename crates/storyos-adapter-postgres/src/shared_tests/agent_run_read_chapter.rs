//! The shared test support from `agent_run_read_chapter_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::chapter_list_context::head;
use crate::shared_tests::command_sequence_structure::{CREATE_CHAPTER, create_chapter};
use crate::shared_tests::command_sequence_support::issued;
use crate::shared_tests::create_volume_authority::{NamedEdit, adjusted_edit_command};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::set_current_chapter_authority::open_session_request;
use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ApplyAuthorEditCommand,
    ChallengeRateClass, CreateChapterInput, EditorSessionStore, ProjectScope,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, CreateChapterPlacement, SelectionSnapshot,
    TransitionOutcome, UTF16_COORDINATE_PROFILE,
};

/// The expected result of a read of the Chapter `number`: the header line and each Block.
pub(crate) fn read_text(number: i64, chapter: (&str, &str, &str), blocks: &[&str]) -> String {
    range_text(number, chapter, (blocks.len(), 1), blocks)
}

/// The expected result of a read of the Blocks `blocks` of the Chapter `number`, which has
/// `count` Blocks, from the Block `first`.
pub(crate) fn range_text(
    number: i64,
    (title, chapter_id, revision_id): (&str, &str, &str),
    (count, first): (usize, usize),
    blocks: &[&str],
) -> String {
    let header = json!({
        "blocks": count,
        "chapter": number,
        "chapter_id": chapter_id,
        "first_block": first,
        "last_block": first + blocks.len() - 1,
        "revision_id": revision_id,
        "title": title,
    });
    let blocks: String = (blocks.iter().enumerate())
        .map(|(index, text)| format!("\n[block {}]\n{text}", first + index))
        .collect();
    format!("{header}{blocks}")
}

pub(crate) fn finish() -> Value {
    json!(["finish", "settled", null, "", true])
}

/// The output of the call `call_id` in its `tool_result` boundary with the state `state`.
pub(crate) fn boundary(call_id: &str, state: &str, text: &str) -> Value {
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
pub(crate) async fn sent(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
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
}

/// The trust and state of each `tool_results` source, and the class, source version, and
/// projection mode of each function output item, of the decision Context Assembly at `position`.
pub(crate) async fn outputs_record(admin: &Client, run_id: &str, position: i32) -> Value {
    json_of(admin, "SELECT jsonb_build_array(
                      (SELECT jsonb_agg(jsonb_build_array(source ->> 'trust', source ->> 'state'))
                         FROM jsonb_array_elements(payload #> '{operation_requirement,tool_results}')
                              AS source),
                      (SELECT jsonb_agg(jsonb_build_array(item ->> 'source_class',
                                                          item ->> 'source_version',
                                                          item ->> 'projection_mode'))
                         FROM jsonb_array_elements(payload -> 'selected') AS item
                        WHERE item ->> 'source_class'
                              IN ('tool_result', 'manuscript_read_result', 'manuscript_search_result',
                                   'memory_read_result', 'memory_search_result')))::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'
                AND decision_position = $2::text::numeric", &[&run_id, &position.to_string()]).await
}

/// The identity of each call of the Run with the function name `name`, in call order, or of
/// each call when `name` is `None`.
pub(crate) async fn call_ids(admin: &Client, run_id: &str, name: Option<&str>) -> Vec<String> {
    admin
        .query(
            "SELECT tool_call_id::text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = COALESCE($2, function_name)
                AND function_name <> 'finish'
              ORDER BY decision_position, call_position",
            &[&run_id, &name],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

/// The Author Edit of the Editor Session `session` that writes `sentences` into the empty first
/// Block of `chapter_id` and splits them into one Block each.
pub(crate) async fn write_blocks(
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
pub(crate) async fn project(
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

/// Whether the public ToolCall list of the Model Attempt of the call `tool_call_id` is empty.
pub(crate) async fn no_public_call(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
    tool_call_id: &str,
) -> bool {
    let attempt_id: String = admin
        .query_one(
            "SELECT model_attempt_id::text FROM storyos.tool_calls
              WHERE tool_call_id = $1::text::uuid",
            &[&tool_call_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let public = store
        .read_agent_run(
            scope,
            run_id,
            &AgentRunReadSelection::ModelAttempt(attempt_id),
        )
        .await
        .unwrap()
        .unwrap()
        .decision;
    matches!(
        public,
        AgentRunDecisionInspect::ToolRequest { ref calls, .. } if calls.is_empty()
    )
}
