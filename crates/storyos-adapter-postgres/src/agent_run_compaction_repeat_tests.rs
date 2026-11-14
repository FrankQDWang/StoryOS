use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    CreatePurpose, NoContractFaults, complete_agent_run,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, ADVISORY_TEXT,
    AgentRunStatus, DeploymentDestination,
};
use tokio_postgres::Client;

use crate::agent_run_compaction::summary::tests::{
    ADVICE, ChangingFake, finish_events, near_threshold, second_run_on_agent_plan,
    second_run_on_agent_plan_after, streamed, with_usage,
};
use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run_in, claim_run, queued_run, stores};
use crate::agent_run_finish_rule::tests::tool_loop_run;

/// An author message that asks the Host fake to compact at RunSteps 0 and 1.
fn compact_twice() -> String {
    format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 0 1. Help with this passage."
    )
}

/// For each compaction record of the Run in RunStep order: its position, its install state, and
/// its refusal reason.
async fn records(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(decision_position, install_state, refusal_reason)::text
               FROM storyos.active_context_compactions
              WHERE run_id = $1::text::uuid
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

/// The status of the Run, the capability of its refusal, and its count of decision Attempts.
async fn settlement(admin: &Client, run_id: &str) -> Value {
    admin
        .query_one(
            "SELECT jsonb_build_array(run.status, run.settlement->>'capability',
                      (SELECT count(*) FROM storyos.model_attempts AS decision
                        WHERE decision.run_id = run.run_id
                          AND decision.attempt_role = 'decision'))::text
               FROM storyos.agent_runs AS run
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn one_run_compacts_at_two_run_steps_and_the_second_summary_continues_the_first() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // RunStep 0 continues the response of the first Run of the Conversation. RunStep 1 continues
    // the later request of RunStep 0, and it waits for the output of its `finish` call.
    let (scope, _, _, run_id) = tool_loop_run(
        &store,
        &admin,
        ["840", "841"],
        &compact_twice(),
        None,
        /*same_conversation*/ true,
    )
    .await;
    let record = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let rows: Vec<Value> = admin
        .query(
            "SELECT jsonb_build_array(record.decision_position, record.install_state,
                      record.compaction_id, record.prior_model_attempt_id,
                      record.installed_model_attempt_id, later.payload->>'response_reference',
                      wire.later->>'previous_response_reference',
                      wire.summary->>'previous_response_reference',
                      (SELECT jsonb_agg(output->'call_id')
                         FROM jsonb_array_elements(wire.summary->'tool_outputs') AS output),
                      (SELECT jsonb_agg(item->>'source_version')
                         FROM jsonb_array_elements(assembly.payload->'selected') AS item
                        WHERE item->>'source_class' = 'conversation_summary'))::text
               FROM storyos.active_context_compactions AS record
               JOIN storyos.model_attempts AS later
                 ON later.model_attempt_id = record.installed_model_attempt_id
               JOIN storyos.model_attempts AS summary
                 ON summary.model_attempt_id = record.producer_model_attempt_id
               JOIN storyos.context_assembly_manifests AS assembly
                 ON assembly.destination_context_manifest_id =
                    later.destination_context_manifest_id,
                    LATERAL (SELECT (later.payload #>> '{wire,serialized_payload}')::jsonb,
                                    (summary.payload #>> '{wire,serialized_payload}')::jsonb)
                      AS wire(later, summary)
              WHERE record.run_id = $1::text::uuid
              ORDER BY record.decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect();
    // Each later request starts a new transport and sends only the summary of its own record.
    let facts = |row: &Value| {
        (
            row[0].clone(),
            row[1].clone(),
            row[6].is_null(),
            row[9] == json!([row[2]]),
        )
    };

    assert_eq!(
        (
            rows.iter().map(facts).collect::<Vec<_>>(),
            // The second summary request continues the response of the first later request, so
            // it summarizes the Provider conversation that holds the first summary.
            rows[1][3] == rows[0][4],
            rows[1][7] == rows[0][5],
            rows[1][8].clone(),
            record.compactions.len(),
            record.status,
            matches!(record.decision, AgentRunDecisionInspect::Advisory { ref text, .. }
                if text == ADVISORY_TEXT),
        ),
        (
            vec![
                (json!(0), json!("installed"), true, true),
                (json!(1), json!("installed"), true, true),
            ],
            true,
            true,
            json!(["call_finish"]),
            2,
            AgentRunStatus::Completed,
            true,
        )
    );
}

/// Settles a first AgentRun and then an AgentRun that compacts at RunSteps 0 and 1 in the same
/// Conversation. The admin connection runs `statement`, with `{project}`, `{chapter}`, and
/// `{run}`, at the first preparation of the second summary request. Returns the compaction
/// records, the input mapping of the decision request of RunStep 1, and the Run status.
async fn compact_twice_with_change(prefixes: [&str; 2], statement: &str) -> Value {
    let (store, admin) = stores().await;
    let destination = DeploymentDestination::HostFakeToolLoop;
    let (scope, chapter_id, first) = queued_run(&store, prefixes[0], destination.clone()).await;
    let fake = storyos_adapter_fake_destination::FakeDestination;
    let claim = claim_run(&admin, scope.clone(), first).await;
    complete_agent_run(&store, &fake, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let run_id = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        prefixes[1],
        destination,
        &compact_twice(),
        ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        },
        None,
    )
    .await;
    let adapter = ChangingFake {
        admin: &admin,
        // Only the second summary request answers a call: the `finish` call of RunStep 0.
        at: |create| {
            create.purpose == CreatePurpose::CompactionSummary && !create.tool_outputs.is_empty()
        },
        statement: statement
            .replace("{project}", scope.project_id.as_ref())
            .replace("{chapter}", &chapter_id)
            .replace("{run}", &run_id),
        done: std::sync::Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let mapping: Option<String> = admin
        .query_one(
            "SELECT payload->'continuation'->>'input_mapping' FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                AND decision_position = 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    json!([
        records(&admin, &run_id).await,
        mapping,
        settlement(&admin, &run_id).await[0]
    ])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_install_check_refuses_a_changed_input_or_a_restricted_source_at_the_second_compaction()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let changed_input = "UPDATE storyos.authoritative_payloads AS payload
            SET canonical_bytes = convert_to('An edit during the second summary.', 'UTF8')
           FROM storyos.authoritative_heads AS head
           JOIN storyos.authoritative_revisions AS revision
             ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                 revision.revision_id) =
                (head.owner_user_id, head.project_id, head.manuscript_object_id,
                 head.current_revision_id)
          WHERE (payload.owner_user_id, payload.project_id, payload.payload_id) =
                (revision.owner_user_id, revision.project_id, revision.payload_id)
            AND head.project_id = '{project}'::uuid
            AND head.manuscript_object_id = '{chapter}'::uuid";
    // The source of the second summary is the later request of RunStep 0.
    let restricted_source = r#"UPDATE storyos.model_attempts
            SET payload = payload || '{"covered_copy_restricted": true}'
          WHERE run_id = '{run}'::uuid AND attempt_role = 'decision'
            AND decision_position = 0"#;
    let cases = [
        (
            ["842", "843"],
            changed_input,
            "changed_input",
            "incremental",
        ),
        (
            ["844", "845"],
            restricted_source,
            "restricted_source",
            "new_transport",
        ),
    ];
    for (prefixes, statement, reason, mapping) in cases {
        assert_eq!(
            compact_twice_with_change(prefixes, statement).await,
            json!([
                [[0, "installed", null], [1, "refused", reason]],
                mapping,
                "completed"
            ]),
            "{reason}"
        );
    }
}

const FIRST_SUMMARY: &str = "The author keeps the rain scene.";
const SECOND_SUMMARY: &str = "The author keeps the rain scene and the open letter.";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_that_crosses_the_threshold_two_times_compacts_two_times() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // RunStep 0 compacts: the closing response of the first Run puts the new turn above the
    // threshold of 812,646 tokens. The later request is a full input, and its response reports
    // its usage. RunStep 1 sends only the output of the `finish` call of that response, because
    // that response read the author message of 4,800 tokens.
    let message = "Help again. ".repeat(/*n*/ 400);
    let cases = [
        // 810,646 tokens and the short output are below the threshold.
        (["846", "847"], 810_546, None),
        // 812,646 tokens and the output are above the threshold, so RunStep 1 compacts again.
        (["848", "849"], 812_546, Some(SECOND_SUMMARY)),
    ];
    for (prefixes, input_tokens, second) in cases {
        let closing = match second {
            Some(summary) => vec![
                streamed("resp_summary_2", summary),
                streamed("resp_later_closing", ADVICE),
            ],
            None => vec![streamed("resp_closing", ADVICE)],
        };
        let replies = [
            near_threshold().to_vec(),
            vec![
                streamed("resp_summary", FIRST_SUMMARY),
                with_usage(
                    finish_events("resp_later"),
                    input_tokens,
                    /*output*/ 100,
                ),
            ],
            closing,
        ]
        .concat();
        let run_id = second_run_on_agent_plan(&store, &admin, prefixes, &message, replies).await;
        let sent: Value = admin
            .query_one(
                "SELECT jsonb_build_array(
                          (summary.payload #>> '{wire,serialized_payload}')::jsonb
                            ->> 'previous_response_id',
                          (later.payload #>> '{wire,serialized_payload}')::jsonb
                            ? 'previous_response_id',
                          (later.payload #>> '{wire,serialized_payload}')::jsonb
                            #>> '{input,0,content,0,text}')::text
                   FROM storyos.model_attempts AS later
                   LEFT JOIN storyos.model_attempts AS summary
                     ON summary.run_id = later.run_id AND summary.attempt_role = 'compaction'
                    AND summary.decision_position = 1
                  WHERE later.run_id = $1::text::uuid AND later.attempt_role = 'decision'
                    AND later.decision_position = 1",
                &[&run_id],
            )
            .await
            .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
            .unwrap();
        let first_text = sent[2].as_str().unwrap_or_default();
        let mut expected_records = vec![json!([0, "installed", null])];
        if second.is_some() {
            expected_records.push(json!([1, "installed", null]));
        }

        assert_eq!(
            (
                records(&admin, &run_id).await,
                settlement(&admin, &run_id).await,
                // The second summary request continues the response of the first later request.
                // The second later request starts a new transport with the second summary only.
                sent[0].clone(),
                sent[1].clone(),
                first_text.starts_with("Summary of the earlier conversation:")
                    && first_text.contains(SECOND_SUMMARY),
            ),
            (
                expected_records,
                json!(["completed", null, 2]),
                json!(second.map(|_| "resp_later")),
                json!(second.is_none()),
                second.is_some(),
            ),
            "{prefixes:?}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_that_a_compaction_cannot_fit_ends_as_too_long() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The window after the output bound is 1,048,576 - 32,768 = 1,015,808 tokens.
    let mut dropped = streamed("resp_dropped", "The author keeps the rain.");
    dropped.truncate(/*len*/ 3);
    let first_compaction = || {
        [
            near_threshold().to_vec(),
            vec![streamed("resp_summary", FIRST_SUMMARY)],
        ]
        .concat()
    };
    let later = |input_tokens| {
        with_usage(
            finish_events("resp_later"),
            input_tokens,
            /*output*/ 100,
        )
    };
    // The count stands for a long input of RunStep 0. With the summary, the full input of the
    // later request is above the window.
    let long_input = "UPDATE storyos.context_assembly_manifests
            SET payload = jsonb_set(payload, '{selected,0,token_count}', '1015808')
          WHERE run_id = '{run}'::uuid AND manifest_role = 'decision'
            AND decision_position = 0";
    let cases = [
        // The second summary request supplies no summary, and the usage is at the window.
        (
            ["84a", "84b"],
            [first_compaction(), vec![later(1_015_800), dropped]].concat(),
            "",
            json!([
                [0, "installed", null],
                [1, "refused", "summary_unavailable"]
            ]),
            json!(["refused", "conversation_too_long", 1]),
        ),
        // The second summary is over the item limit. The usage is below the window, so RunStep 1
        // continues the response with no compaction.
        (
            ["84c", "84d"],
            [
                first_compaction(),
                vec![
                    later(812_546),
                    streamed("resp_long", &"a".repeat(/*n*/ 32_001)),
                    streamed("resp_closing", ADVICE),
                ],
            ]
            .concat(),
            "",
            json!([
                [0, "installed", null],
                [1, "refused", "summary_over_item_limit"]
            ]),
            json!(["completed", null, 2]),
        ),
        // The summary installs, and the full input of the later request is still at the window.
        (
            ["84e", "84f"],
            first_compaction(),
            long_input,
            json!([[0, "staged", null]]),
            json!(["refused", "conversation_too_long", 0]),
        ),
    ];
    for (prefixes, replies, before_claim, expected_records, expected_settlement) in cases {
        let run_id = second_run_on_agent_plan_after(
            &store,
            &admin,
            prefixes,
            "Help again.",
            replies,
            before_claim,
        )
        .await;

        assert_eq!(
            (
                Value::from(records(&admin, &run_id).await),
                settlement(&admin, &run_id).await
            ),
            (expected_records, expected_settlement),
            "{prefixes:?}"
        );
    }
}
