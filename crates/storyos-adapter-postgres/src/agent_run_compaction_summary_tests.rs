use storyos_application::{
    ConversationSelection, ModelProviderAdapter, NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::{ACTIVE_COMPACTION_REQUEST_PREFIX, DeploymentDestination};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_compaction_summary::{
    ADVICE, ChangingFake, finish_events, near_threshold, second_run_on_agent_plan, streamed,
    with_usage,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::RuntimeClock;
use crate::shared_tests::model_gateway::{Admission, claim_run, queued_run};

/// Settles the first AgentRun of a new Conversation, then settles one compaction request in the
/// same Conversation, and returns the identity of the second AgentRun.
async fn compact_after_first_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    destination: DeploymentDestination,
    adapter: &impl ModelProviderAdapter,
    before_compaction: &str,
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) = queued_run(store, prefixes[0], destination.clone()).await;
    let claim = claim_run(admin, scope.clone(), first).await;
    complete_agent_run(store, adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let run_id = Admission {
        destination,
        message: ACTIVE_COMPACTION_REQUEST_PREFIX,
        conversation: ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        },
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, prefixes[1])
    .await;
    admin
        .batch_execute(&before_compaction.replace("{project}", scope.project_id.as_ref()))
        .await
        .unwrap();
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(store, adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    (scope, run_id)
}

/// The run status, the input mapping of the first decision Attempt, the compaction Attempt
/// count, and the compaction record of one AgentRun.
async fn compaction_facts(admin: &Client, run_id: &str) -> [Option<String>; 6] {
    let row = admin
        .query_one(
            "SELECT run.status, decision.payload->'continuation'->>'input_mapping',
                    (SELECT count(*)::text FROM storyos.model_attempts AS summary
                      WHERE summary.run_id = run.run_id AND summary.attempt_role = 'compaction'),
                    record.install_state, record.producer, record.output_text
               FROM storyos.agent_runs AS run
               LEFT JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
               LEFT JOIN storyos.active_context_compactions AS record
                 ON record.run_id = run.run_id
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_summary_outside_the_pinned_grant_scope_leaves_the_ordinary_continuation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The pinned grant has the first scope, which does not name the compaction Purpose.
    let (_, run_id) = compact_after_first_run(
        &store,
        &admin,
        ["ec1", "ec2"],
        DeploymentDestination::HostFakeToolLoop,
        &storyos_adapter_fake_destination::FakeDestination,
        "SET session_replication_role = replica;
         UPDATE storyos.project_destination_grants
            SET grant_scope = 'project_writing_assistance_v1'
          WHERE project_id = '{project}';
         SET session_replication_role = DEFAULT;",
    )
    .await;

    assert_eq!(
        compaction_facts(&admin, &run_id).await,
        [
            // The first request sends the `chapter_list` item, a source that the first scope
            // does not name, so the Run refuses before its first decision (ADR 0095). A binding
            // on the tool loop head always records a grant with the current scope.
            Some("refused".to_owned()),
            None,
            Some("0".to_owned()),
            None,
            None,
            None,
        ]
    );
    let settlement: Option<String> = admin
        .query_one(
            "SELECT settlement::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        settlement.as_deref(),
        Some(r#"{"kind": "execution_refused", "capability": "disclosure_outside_grant"}"#)
    );
}

/// Settles the first AgentRun of a new Conversation on a local scripted Agent Plan destination
/// that answers with `replies` in order. Then settles one more request in the same
/// Conversation, and returns the identity of the second AgentRun.
async fn compact_on_agent_plan(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    replies: Vec<Vec<serde_json::Value>>,
) -> String {
    second_run_on_agent_plan(store, admin, prefixes, "Help again.", replies).await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_streamed_agent_plan_summary_installs_for_the_later_decision_request() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let summary = "The author keeps the rain scene and the open letter.";
    let run_id = compact_on_agent_plan(
        &store,
        &admin,
        ["ec3", "ec4"],
        [
            near_threshold().to_vec(),
            vec![
                streamed("resp_summary", summary),
                finish_events("resp_later"),
                streamed("resp_later_closing", ADVICE),
            ],
        ]
        .concat(),
    )
    .await;
    let later = admin
        .query_one(
            "SELECT summary.payload->>'response_reference',
                    (summary.payload->'wire'->>'serialized_payload')::jsonb
                      ->>'previous_response_id',
                    (decision.payload->'wire'->>'serialized_payload')::jsonb
                      ? 'previous_response_id',
                    strpos((decision.payload->'wire'->>'serialized_payload')::jsonb
                             #>> '{input,0,content,0,text}',
                           'Summary of the earlier conversation:') = 1,
                    record.installed_model_attempt_id = decision.model_attempt_id
                      AND record.installed_model_invocation_id = decision.model_invocation_id
                      AND record.installed_run_step_id IS NOT NULL,
                    assembly.manifest_role, assembly.payload->'selected'->0->>'projection_mode'
               FROM storyos.model_attempts AS summary
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = summary.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
               JOIN storyos.active_context_compactions AS record
                 ON record.run_id = summary.run_id
               JOIN storyos.context_assembly_manifests AS assembly
                 ON assembly.destination_context_manifest_id =
                    decision.destination_context_manifest_id
              WHERE summary.run_id = $1::text::uuid AND summary.attempt_role = 'compaction'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            compaction_facts(&admin, &run_id).await,
            later.get::<_, Option<String>>(/*idx*/ 0),
            later.get::<_, Option<String>>(/*idx*/ 1),
            [2, 3, 4].map(|index| later.get::<_, bool>(index)),
            later.get::<_, String>(/*idx*/ 5),
            later.get::<_, Option<String>>(/*idx*/ 6),
        ),
        (
            [
                Some("completed".to_owned()),
                Some("new_transport".to_owned()),
                Some("1".to_owned()),
                Some("installed".to_owned()),
                Some("agent_plan_summary".to_owned()),
                Some(summary.to_owned()),
            ],
            Some("resp_summary".to_owned()),
            Some("resp_decision".to_owned()),
            [false, true, true],
            "later_request".to_owned(),
            Some("derived_summary".to_owned()),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_dropped_summary_stream_keeps_its_items_and_the_decision_continues_the_prior_response() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The summary stream ends after its first delta, with no terminal event.
    let mut dropped = streamed("resp_dropped", "The author keeps the rain.");
    dropped.truncate(/*len*/ 3);
    let run_id = compact_on_agent_plan(
        &store,
        &admin,
        ["ec5", "ec6"],
        [
            near_threshold().to_vec(),
            vec![
                dropped,
                finish_events("resp_next"),
                streamed("resp_next_closing", ADVICE),
            ],
        ]
        .concat(),
    )
    .await;
    let summary_attempt = admin
        .query_one(
            "SELECT attempt.payload->>'response_reference', attempt.payload->'items'->0->>'text',
                    attempt.payload->>'observation', record.refusal_reason,
                    (decision.payload->'wire'->>'serialized_payload')::jsonb
                      ->>'previous_response_id'
               FROM storyos.model_attempts AS attempt
               JOIN storyos.active_context_compactions AS record ON record.run_id = attempt.run_id
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = attempt.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'compaction'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        [0, 1, 2, 3, 4].map(|index| summary_attempt.get::<_, Option<String>>(index)),
        [
            Some("resp_dropped".to_owned()),
            Some("The author keeps the rain.".to_owned()),
            Some("outcome_unknown".to_owned()),
            Some("summary_unavailable".to_owned()),
            Some("resp_decision".to_owned()),
        ]
    );
}

/// Settles a first AgentRun and then a compaction request in the same Conversation, with the
/// concurrent change of `adapter`. Returns the compaction refusal, the input mapping of the
/// decision Attempt, and the role of the Context Assembly Manifest that its request disclosed.
async fn compact_with_change(
    prefixes: [&str; 2],
    at: fn(&storyos_application::CreateRequest) -> bool,
    statement: &str,
) -> [Option<String>; 4] {
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) =
        queued_run(&store, prefixes[0], DeploymentDestination::HostFakeToolLoop).await;
    let claim = claim_run(&admin, scope.clone(), first.clone()).await;
    complete_agent_run(
        &store,
        &storyos_adapter_fake_destination::FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let run_id = Admission {
        message: ACTIVE_COMPACTION_REQUEST_PREFIX,
        conversation: ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        },
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, prefixes[1])
    .await;
    let adapter = ChangingFake {
        admin: &admin,
        at,
        statement: statement
            .replace("{project}", scope.project_id.as_ref())
            .replace("{chapter}", &chapter_id)
            .replace("{first}", &first),
        done: std::sync::Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let row = admin
        .query_one(
            "SELECT record.install_state, record.refusal_reason,
                    decision.payload->'continuation'->>'input_mapping', assembly.manifest_role
               FROM storyos.active_context_compactions AS record
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = record.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
               JOIN storyos.context_assembly_manifests AS assembly
                 ON assembly.destination_context_manifest_id =
                    decision.destination_context_manifest_id
              WHERE record.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_edit_during_the_summary_request_refuses_the_install() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    // The Working Target changes after the decision Context Assembly and before the summary
    // returns. The later request copies that assembly, so the install must refuse.
    let facts = compact_with_change(
        ["ec7", "ec8"],
        |create| create.purpose == storyos_application::CreatePurpose::CompactionSummary,
        "UPDATE storyos.authoritative_payloads AS payload
            SET canonical_bytes = convert_to('An edit during the summary.', 'UTF8')
           FROM storyos.authoritative_heads AS head
           JOIN storyos.authoritative_revisions AS revision
             ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                 revision.revision_id) =
                (head.owner_user_id, head.project_id, head.manuscript_object_id,
                 head.current_revision_id)
          WHERE (payload.owner_user_id, payload.project_id, payload.payload_id) =
                (revision.owner_user_id, revision.project_id, revision.payload_id)
            AND head.project_id = '{project}'::uuid
            AND head.manuscript_object_id = '{chapter}'::uuid",
    )
    .await;

    assert_eq!(
        facts,
        [
            Some("refused".to_owned()),
            Some("changed_input".to_owned()),
            Some("incremental".to_owned()),
            Some("decision".to_owned()),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_source_restricted_after_the_install_check_keeps_the_summary_unsent() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    // The prior Attempt becomes restricted while the later request is prepared, before its
    // dispatch claim.
    let facts = compact_with_change(
        ["ec9", "eca"],
        |create| {
            create.purpose == storyos_application::CreatePurpose::Decision
                && create.context.first().map(|item| item.source_class)
                    == Some(storyos_core::ContextSourceClass::ConversationSummary)
        },
        "UPDATE storyos.model_attempts
            SET payload = jsonb_set(payload, '{produced_binding,covered_copy_restricted}', 'true')
          WHERE run_id = '{first}'::uuid AND attempt_role = 'decision'",
    )
    .await;

    assert_eq!(
        facts,
        [
            Some("refused".to_owned()),
            Some("restricted_source".to_owned()),
            Some("new_transport".to_owned()),
            Some("decision".to_owned()),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_chain_below_the_threshold_continues_with_no_compaction() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The first response reports 52 tokens, far below the threshold of 812,646 tokens.
    let run_id = compact_on_agent_plan(
        &store,
        &admin,
        ["ecb", "ecc"],
        vec![
            streamed("resp_decision", ADVICE),
            streamed("resp_next", ADVICE),
        ],
    )
    .await;

    assert_eq!(
        compaction_facts(&admin, &run_id).await,
        [
            Some("completed".to_owned()),
            Some("incremental".to_owned()),
            Some("0".to_owned()),
            None,
            None,
            None,
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_chain_at_the_window_without_an_installed_summary_refuses_as_too_long() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The closing response of the first Run reports 1,015,900 tokens. The window after the output bound is
    // 1,015,808 tokens. The summary stream drops, so no summary installs.
    let mut dropped = streamed("resp_dropped", "The author keeps the rain.");
    dropped.truncate(/*len*/ 3);
    let run_id = compact_on_agent_plan(
        &store,
        &admin,
        ["ecd", "ece"],
        vec![
            finish_events("resp_finish"),
            with_usage(
                streamed("resp_decision", ADVICE),
                /*input*/ 1_015_800,
                /*output*/ 100,
            ),
            dropped,
        ],
    )
    .await;
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement->>'capability', record.refusal_reason,
                    (SELECT count(*)::text FROM storyos.model_attempts AS decision
                      WHERE decision.run_id = run.run_id AND decision.attempt_role = 'decision')
               FROM storyos.agent_runs AS run
               JOIN storyos.active_context_compactions AS record ON record.run_id = run.run_id
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        [0, 1, 2, 3].map(|index| row.get::<_, Option<String>>(index)),
        [
            Some("refused".to_owned()),
            Some("conversation_too_long".to_owned()),
            Some("summary_unavailable".to_owned()),
            Some("0".to_owned()),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_run_step_counts_only_the_outputs_of_its_prior_response_in_the_expected_input() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The request at position 1 of the second Run sends only the output of the `finish` call of
    // the response at position 0. The reported usage of that response already has the author
    // message of 4,800 tokens. The threshold is 812,646 tokens.
    let message = "Help again. ".repeat(/*n*/ 400);
    let cases = [
        // 810,646 tokens and the short output are below the threshold.
        (["811", "812"], 810_546, None),
        // 812,645 tokens are one token below the threshold, and the output puts them above it.
        (["813", "814"], 812_545, Some("installed")),
    ];
    for (prefixes, input_tokens, install_state) in cases {
        let run_id = second_run_on_agent_plan(
            &store,
            &admin,
            prefixes,
            &message,
            vec![
                finish_events("resp_finish"),
                streamed("resp_decision", ADVICE),
                with_usage(
                    finish_events("resp_again"),
                    input_tokens,
                    /*output*/ 100,
                ),
                streamed("resp_closing", ADVICE),
                streamed("resp_later_closing", ADVICE),
            ],
        )
        .await;
        let facts = compaction_facts(&admin, &run_id).await;

        assert_eq!(
            [&facts[0], &facts[2], &facts[3]],
            [
                &Some("completed".to_owned()),
                &Some(u8::from(install_state.is_some()).to_string()),
                &install_state.map(str::to_owned),
            ],
            "{prefixes:?}"
        );
    }
}
