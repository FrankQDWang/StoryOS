use serde_json::{Value, json};
use storyos_application::{AgentRunReadSelection, AgentRunReadStore, ConversationSelection};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, AgentRunStatus,
    DeploymentDestination, HOST_FAKE_COMPACTION_OUTPUT,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{admit_run_in, stores};
use crate::agent_run_finish_rule::tests::{complete, tool_loop_run};
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle, steering};

/// For each decision request of the Run in RunStep order: its position, the earlier author
/// messages and the author message that it sent, and whether it sent the current input again
/// with the replaced input label.
async fn sent(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(decision_position,
                      wire->'earlier_author_messages', wire->>'author_message',
                      wire->'replaces_earlier_input') ORDER BY decision_position)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// For each decision and later request Context Assembly of the Run in RunStep order: its role,
/// its position, the text of each selected `conversation_summary` and `author_instruction` item,
/// and the earlier author messages that its Operation Requirement records.
async fn assemblies(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(manifest_role, decision_position,
                      (SELECT jsonb_agg(jsonb_build_array(item->>'source_class',
                                                          item->>'content'))
                         FROM jsonb_array_elements(payload->'selected') AS item
                        WHERE item->>'source_class' IN ('conversation_summary',
                                                        'author_instruction')),
                      payload #> '{operation_requirement,earlier_author_messages}')
                      ORDER BY decision_position, manifest_role)::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role IN ('decision', 'later_request')",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

fn source(run_id: &str, message_id: &str) -> Value {
    json!({"run_id": run_id, "message_id": message_id})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_later_request_sends_the_earlier_author_messages_of_the_conversation_in_exact_words() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The first Run reads a steering input at RunStep 1. Two more Runs follow it in the same
    // Conversation, and the fourth Run compacts at RunStep 0.
    let (scope, chapter_id, first) = queued(&store, "860").await;
    let input = steering(&store, &scope, 0x8200, &first, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &first, &author)
        .await
        .unwrap();
    let conversation = ConversationSelection::Existing {
        conversation_id: "018f0000-0000-7001-8000-000000068606".to_owned(),
    };
    let current = format!("{ACTIVE_COMPACTION_REQUEST_PREFIX} Help again.");
    let mut runs = vec![first];
    for (prefix, message) in [
        ("861", "Make Mara older."),
        ("862", "Name the inn The Gull."),
        ("863", current.as_str()),
    ] {
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefix,
            DeploymentDestination::HostFakeToolLoop,
            message,
            conversation.clone(),
            None,
        )
        .await;
        complete(&store, &admin, &scope, &run_id).await;
        runs.push(run_id);
    }
    let first_read = store
        .read_agent_run(&scope, &runs[0], &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let steered = first_read.steering_inputs[0].steering_input_id.clone();
    let read = store
        .read_agent_run(&scope, &runs[3], &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let earlier = [
        "Help with this passage.",
        "Keep the voice.",
        "Make Mara older.",
        "Name the inn The Gull.",
    ];
    let author = |text: &str| json!(["author_instruction", text]);

    assert_eq!(
        (
            read.status,
            read.compactions
                .iter()
                .map(|record| record.kept_author_message_ids.clone())
                .collect::<Vec<_>>(),
            assemblies(&admin, &runs[3]).await,
            sent(&admin, &runs[3]).await,
        ),
        (
            AgentRunStatus::Completed,
            vec![vec![
                runs[0].clone(),
                steered.clone(),
                runs[1].clone(),
                runs[2].clone(),
            ]],
            // The later request selects the earlier messages after the summary and before the
            // current input. The current input stays one item, and the RunStep after the later
            // request reads exactly the current message.
            json!([
                ["decision", 0, [author(&current)], null],
                [
                    "later_request",
                    0,
                    [
                        ["conversation_summary", HOST_FAKE_COMPACTION_OUTPUT],
                        author(earlier[0]),
                        author(earlier[1]),
                        author(earlier[2]),
                        author(earlier[3]),
                        author(&current)
                    ],
                    [
                        source(&runs[0], &runs[0]),
                        source(&runs[0], &steered),
                        source(&runs[1], &runs[1]),
                        source(&runs[2], &runs[2])
                    ]
                ],
                ["decision", 1, [author(&current)], null],
            ]),
            // The RunStep after the later request continues its response with an unchanged
            // input, so it sends no current input and no earlier message.
            json!([[0, earlier, current, null], [1, null, null, null]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_compaction_in_the_first_run_of_a_conversation_sends_no_earlier_author_message() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The Run of another Conversation of the Project completes first.
    let current = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let (scope, _, _, run_id) = tool_loop_run(
        &store,
        &admin,
        ["864", "865"],
        &current,
        None,
        /*same_conversation*/ false,
    )
    .await;
    let read = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        (
            read.compactions
                .iter()
                .map(|record| record.kept_author_message_ids.clone())
                .collect::<Vec<_>>(),
            sent(&admin, &run_id).await,
        ),
        (
            vec![Vec::<String>::new()],
            json!([[0, null, current, null], [1, null, current, null]]),
        )
    );
}
