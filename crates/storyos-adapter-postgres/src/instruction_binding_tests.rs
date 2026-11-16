//! The Project Instruction Binding of each AgentRun and the requests that send its text (ADR 0094
//! section 2, ADR 0085, ADR 0093).

use serde_json::{Value, json};
use storyos_application::{ConversationSelection, ProjectScope};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, DeploymentDestination,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_run, admit_run_in, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle, steering};
use crate::project_instruction_tests::{applied, save};

const FAKE: DeploymentDestination = DeploymentDestination::HostFakeToolLoop;

/// Saves `text` as Revision `expected + 1` with the challenge `suffix`, and gives its identity.
async fn saved(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    text: &str,
    expected: u64,
) -> String {
    applied(&save(store, scope, suffix, text, expected).await).instruction_revision_id
}

/// The binding column of the Run `run_id`.
async fn binding(admin: &Client, run_id: &str) -> Option<String> {
    admin
        .query_one(
            "SELECT project_instruction_revision_id::text FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// For each Context Assembly of the Run in the order of its creation: its role, its RunStep, its
/// Operation Requirement value of the instruction, the version of each considered
/// `instruction_binding` source, and the source class of each selected item.
async fn assemblies(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(manifest_role, decision_position,
                      payload #> '{operation_requirement,instruction}',
                      jsonb_path_query_array(payload,
                        '$.considered[*] ? (@.source_class == \"instruction_binding\").source_version'),
                      jsonb_path_query_array(payload, '$.selected[*].source_class'))
                      ORDER BY context_assembly_manifest_id)::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, and
/// the text of each `instruction_binding` item that the fake destination received.
async fn sent(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position,
                      jsonb_path_query_array(wire,
                        '$.context[*] ? (@.source_class == \"instruction_binding\").content'))
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

/// The Conversation of the Run that `admit_run` admitted with `prefix`.
fn conversation_of(prefix: &str) -> ConversationSelection {
    ConversationSelection::Existing {
        conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
    }
}

fn required(revision_id: &str) -> Value {
    json!({"kind": "required_revision", "revision_id": revision_id, "available": true})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_binds_only_a_current_revision_with_text_other_than_white_space() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, none) = queued(&store, "db1").await;
    let blank = saved(&store, &scope, 0xdd11, " \n\u{3000}", 0).await;
    let white_space = admit_run(&store, &scope, &chapter_id, "db2", FAKE, "Help again.").await;
    let text = " Write in the past tense.\n";
    let current = saved(&store, &scope, 0xdd12, text, 1).await;
    let bound = admit_run(&store, &scope, &chapter_id, "db3", FAKE, "Help again.").await;
    let context = |run_id: String| {
        let admin = &admin;
        async move { assemblies(admin, &run_id).await }
    };

    assert_ne!(blank, current);
    assert_eq!(
        (
            binding(&admin, &none).await,
            binding(&admin, &white_space).await,
            binding(&admin, &bound).await,
            context(none).await,
            context(white_space).await,
            context(bound.clone()).await,
        ),
        (
            None,
            None,
            Some(current.clone()),
            json!([["decision", 0, {"kind": "absent"}, ["absent"],
                    ["author_instruction", "working_target"]]]),
            json!([["decision", 0, {"kind": "absent"}, ["absent"],
                    ["author_instruction", "working_target"]]]),
            json!([[
                "decision",
                0,
                required(&current),
                [current],
                [
                    "author_instruction",
                    "instruction_binding",
                    "working_target"
                ]
            ]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_binds_the_revision_of_its_own_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (project_a, chapter_a, _) = queued(&store, "db4").await;
    let (project_b, _, _) = queued(&store, "db5").await;
    let revision_a = saved(&store, &project_a, 0xdd41, "Rule of Project A.", 0).await;
    saved(&store, &project_b, 0xdd51, "Rule of Project B.", 0).await;
    let run_id = admit_run(&store, &project_a, &chapter_a, "db6", FAKE, "Help again.").await;
    let payloads: String = admin
        .query_one(
            "SELECT string_agg(payload::text, ' ') FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            binding(&admin, &run_id).await,
            payloads.contains("Rule of Project A."),
            payloads.contains("Rule of Project B."),
        ),
        (Some(revision_a), true, false)
    );
}

/// The version of each considered `instruction_binding` source of each Context Assembly of the
/// Run, with the role and the RunStep of the assembly.
async fn versions(admin: &Client, run_id: &str) -> Vec<Value> {
    assemblies(admin, run_id)
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|assembly| json!([assembly[0], assembly[1], assembly[3]]))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_keeps_its_revision_and_the_next_run_binds_the_current_one() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "db7").await;
    complete(&store, &admin, &scope, &first).await;
    let first_text = "Write in the past tense.";
    let first_revision = saved(&store, &scope, 0xdd71, first_text, 0).await;
    let steered = admit_run(&store, &scope, &chapter_id, "db8", FAKE, "Help again.").await;
    // The author saves a new text after the admission of the Run.
    let next_text = "Write in the present tense.";
    let next_revision = saved(&store, &scope, 0xdd72, next_text, 1).await;
    let input = steering(&store, &scope, 0xdde1, &steered, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &steered, &author)
        .await
        .unwrap();
    let next = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "db9",
        FAKE,
        "Help again.",
        conversation_of("db8"),
        None,
    )
    .await;
    complete(&store, &admin, &scope, &next).await;

    assert_eq!(
        (
            versions(&admin, &steered).await,
            sent(&admin, &steered).await,
            binding(&admin, &next).await,
            sent(&admin, &next).await,
        ),
        (
            // The RunStep that reads the steering input and the RunStep after the Tool result
            // keep the first Revision.
            vec![
                json!(["decision", 0, [first_revision]]),
                json!(["decision", 1, [first_revision]]),
                json!(["decision", 2, [first_revision]]),
            ],
            // The first request and the request with the replaced input send the text. The
            // incremental request with an unchanged input does not.
            json!([
                ["decision", 0, [first_text]],
                ["decision", 1, [first_text]],
                ["decision", 2, []],
            ]),
            Some(next_revision),
            json!([["decision", 0, [next_text]], ["decision", 1, []]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_after_a_removal_sends_no_project_instruction() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "dbd").await;
    complete(&store, &admin, &scope, &first).await;
    let text = "Write in the past tense.";
    saved(&store, &scope, 0xddd1, text, 0).await;
    let bound = admit_run(&store, &scope, &chapter_id, "dbe", FAKE, "Help again.").await;
    complete(&store, &admin, &scope, &bound).await;
    // The author removes the Project Instruction.
    saved(&store, &scope, 0xddd2, "", 1).await;
    let removed = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "dbf",
        FAKE,
        "Help again.",
        conversation_of("dbe"),
        None,
    )
    .await;
    complete(&store, &admin, &scope, &removed).await;

    assert_eq!(
        (
            sent(&admin, &bound).await,
            binding(&admin, &removed).await,
            sent(&admin, &removed).await,
        ),
        (
            json!([["decision", 0, [text]], ["decision", 1, []]]),
            None,
            json!([["decision", 0, []], ["decision", 1, []]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_later_request_of_a_compaction_sends_the_bound_text_and_the_summary_request_does_not() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "dbb").await;
    complete(&store, &admin, &scope, &first).await;
    let text = "Write in the past tense.";
    let revision = saved(&store, &scope, 0xddb1, text, 0).await;
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let compacted = admit_run(&store, &scope, &chapter_id, "dbc", FAKE, &message).await;
    // The author saves a new text after the admission of the Run.
    saved(&store, &scope, 0xddb2, "Write in the present tense.", 1).await;
    complete(&store, &admin, &scope, &compacted).await;
    let later = assemblies(&admin, &compacted).await[3].clone();

    assert_eq!(
        (
            versions(&admin, &compacted).await,
            later,
            sent(&admin, &compacted).await
        ),
        (
            vec![
                json!(["decision", 0, [revision]]),
                json!(["decision", 1, [revision]]),
                json!(["compaction", 1, []]),
                json!(["later_request", 1, [revision]]),
            ],
            json!([
                "later_request",
                1,
                required(&revision),
                [revision],
                [
                    "conversation_summary",
                    "author_instruction",
                    "instruction_binding",
                    "working_target",
                    "tool_result"
                ]
            ]),
            json!([
                ["decision", 0, [text]],
                ["compaction", 1, []],
                ["decision", 1, [text]],
            ]),
        )
    );
}
