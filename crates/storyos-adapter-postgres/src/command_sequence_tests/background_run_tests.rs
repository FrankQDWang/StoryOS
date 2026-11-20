//! A background Run (ADR 0094 section 5) is a row of `agent_runs` with no Project Conversation, no
//! Conversation Memory Settings revision, no Chapter, no author message, and no Receipt. No read
//! and no command of the author gets it, and the Worker does not claim it. No code starts one
//! yet, so each test inserts it with the test administrator connection.

use serde_json::{Value, json};
use storyos_application::{
    AgentRunControlRefusal, AgentRunReadSelection, AgentRunReadStore, CancelAgentRunInput,
    ConversationSelection, PauseAgentRunInput, ProjectConversationReader, ProjectScope,
    RefusableCommandError, ResearchSourceReadStore, SteerAgentRunInput,
};
use storyos_core::{ACTIVE_COMPACTION_REQUEST_PREFIX, DeploymentDestination};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_sequence::settle_project_command;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::model_gateway::admit_run_in;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_agent_run::{
    CANCEL_AGENT_RUN, PAUSE_AGENT_RUN, create_agent_run_call, request_rows,
};
use crate::shared_tests::command_sequence_steer_agent_run::STEER_AGENT_RUN;
use crate::shared_tests::command_sequence_support::{issued, seed_project, stores};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::archived_rows;

/// Inserts the background Run `run_id` with `status` in the Project of the Run of the author
/// `like`, with the Agent, the grant, and the Model Registration of that Run.
async fn background_run(admin: &Client, like: &str, run_id: &str, status: &str) {
    admin
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, grant_id,
                project_model_use_binding_revision, status, model_registration_revision,
                run_kind)
             SELECT owner_user_id, project_id, $2::text::uuid, project_agent_id, grant_id,
                    project_model_use_binding_revision, $3::text, model_registration_revision,
                    'memory_capture'
               FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&like, &run_id, &status],
        )
        .await
        .unwrap();
}

/// Gives the background Run `run_id` a copy of the primary Operation Requirement, of its Context
/// Assembly, and of the last decision Model Attempt of the Run of the author `like`, with no
/// Receipt and no Conversation. The Model Attempt has new identities, also of its Decision and of
/// its Continuation Binding.
async fn background_records(admin: &Client, like: &str, run_id: &str) {
    admin
        .batch_execute(&format!(
            "WITH requirement AS (
               INSERT INTO storyos.operation_requirements
                 (owner_user_id, project_id, operation_requirement_id, run_id, input_snapshot_id,
                  receipt_id, payload, requirement_role, decision_position)
               SELECT owner_user_id, project_id, gen_random_uuid(), '{run_id}', input_snapshot_id,
                      NULL, payload, requirement_role, decision_position
                 FROM storyos.operation_requirements
                WHERE run_id = '{like}' AND requirement_role = 'primary' AND decision_position = 0
               RETURNING owner_user_id, project_id, operation_requirement_id)
             INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id, operation_requirement_id,
                run_id, sufficiency, payload, receipt_id)
             SELECT owner_user_id, project_id, gen_random_uuid(), operation_requirement_id,
                    '{run_id}', 'complete', '{{}}'::jsonb, NULL
               FROM requirement;
             INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id, model_invocation_id,
                conversation_id, decision_id, continuation_binding_id, dispatch_state, payload)
             SELECT owner_user_id, project_id, '{run_id}', gen_random_uuid(),
                    gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(),
                    gen_random_uuid(), gen_random_uuid(), NULL,
                    CASE WHEN decision_id IS NOT NULL THEN gen_random_uuid() END,
                    CASE WHEN continuation_binding_id IS NOT NULL THEN gen_random_uuid() END,
                    dispatch_state, payload
               FROM storyos.model_attempts
              WHERE run_id = '{like}' AND attempt_role = 'decision'
              ORDER BY decision_position DESC LIMIT 1;"
        ))
        .await
        .unwrap();
}

/// The kind of the Run, and the constraint that refuses each form of `forms` on it.
async fn refusals(admin: &Client, run_id: &str, forms: &[&str]) -> (String, Vec<String>) {
    let mut refused = Vec::new();
    for form in forms {
        let statement =
            format!("UPDATE storyos.agent_runs SET {form} WHERE run_id = '{run_id}'::uuid");
        let error = admin.batch_execute(&statement).await.unwrap_err();
        let database = error.as_db_error().unwrap();
        refused.push(format!(
            "{} {}",
            database.code().code(),
            database.constraint().unwrap()
        ));
    }
    let kind = admin
        .query_one(
            "SELECT run_kind FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    (kind.get(/*idx*/ 0), refused)
}

/// The identity and the kind of each Run in the Project Export Archive of the Project.
async fn archived_kinds(store: &PostgresProjectReader, scope: &ProjectScope) -> Vec<[String; 2]> {
    let [runs] = archived_rows(store, scope, "77", ["agent_runs"]).await;
    runs.iter()
        .map(|run| {
            [&run["run_id"], &run["run_kind"]].map(|value| value.as_str().unwrap().to_owned())
        })
        .collect()
}

/// Counts the Activity events and the Receipts that name the Run.
async fn activity_and_receipts(admin: &Client, run_id: &str) -> [i64; 2] {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE payload->>'run_id' = $1),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE receipt_id = (SELECT receipt_id FROM storyos.agent_runs
                                           WHERE run_id = $1::text::uuid))",
            &[&run_id],
        )
        .await
        .unwrap();
    [row.get(/*idx*/ 0), row.get(/*idx*/ 1)]
}

/// Counts the rows of `agent_runs` with `run_id` that a runtime read in `scope` gets.
async fn visible_rows(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> i64 {
    store
        .in_read_transaction(scope, async |client| {
            let row = client
                .query_one(
                    "SELECT count(*) FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
                    &[&run_id],
                )
                .await
                .unwrap();
            Ok(row.get(/*idx*/ 0))
        })
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_database_check_refuses_each_other_form_of_an_agent_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = create_agent_run_call(&store, /*base*/ 0x7700).await;
    store
        .create_agent_run(&call.envelope, &call.input)
        .await
        .unwrap();
    let scope = call.envelope.project_scope.clone();
    complete(&store, &admin, &scope, &call.input.run_id).await;
    let author = call.input.run_id.as_str();
    let items = [
        "conversation_id",
        "memory_settings_revision",
        "chapter_id",
        "author_message",
        "receipt_id",
    ];
    // A Run of the author without one item, a background Run with one item, and an unknown kind.
    let mut forms: Vec<String> = items.iter().map(|item| format!("{item} = NULL")).collect();
    for kept in items {
        let cleared = items.iter().filter(|item| **item != kept);
        let set = cleared
            .map(|item| format!(", {item} = NULL"))
            .collect::<String>();
        forms.push(format!("run_kind = 'memory_capture'{set}"));
    }
    let cleared = items.map(|item| format!(", {item} = NULL")).concat();
    forms.push(format!("run_kind = 'background'{cleared}"));
    let forms: Vec<&str> = forms.iter().map(String::as_str).collect();
    let observed = refusals(&admin, author, &forms).await;
    // A background Run with none of the five items is a valid form, and its Operation
    // Requirement, its Context Assembly, and its Model Attempt have no Receipt and no Conversation.
    let background = "ffffffff-ffff-7fff-bfff-000000007700";
    background_run(&admin, author, background, "paused").await;
    background_records(&admin, author, background).await;
    let records = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.operation_requirements
                      WHERE run_id = $1::text::uuid AND receipt_id IS NULL),
                    (SELECT count(*) FROM storyos.context_assembly_manifests
                      WHERE run_id = $1::text::uuid AND receipt_id IS NULL),
                    (SELECT count(*) FROM storyos.model_attempts
                      WHERE run_id = $1::text::uuid AND conversation_id IS NULL)",
            &[&background],
        )
        .await
        .unwrap();
    let shape = "23514 agent_runs_kind_shape".to_owned();
    let mut archived = archived_kinds(&store, &scope).await;
    archived.sort();

    assert_eq!(
        (
            observed,
            [0, 1, 2].map(|index| records.get::<_, i64>(index)),
            archived,
        ),
        (
            (
                "author".to_owned(),
                [
                    vec![shape; 10],
                    vec!["23514 agent_runs_run_kind_check".to_owned()]
                ]
                .concat()
            ),
            [1, 1, 1],
            vec![
                [author.to_owned(), "author".to_owned()],
                [background.to_owned(), "memory_capture".to_owned()]
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_background_run_is_not_a_run_that_the_author_can_see_or_control() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = create_agent_run_call(&store, /*base*/ 0x7710).await;
    store
        .create_agent_run(&call.envelope, &call.input)
        .await
        .unwrap();
    let scope = call.envelope.project_scope.clone();
    complete(&store, &admin, &scope, &call.input.run_id).await;
    let (author, conversation_id) = (&call.input.run_id, &call.input.conversation_id);
    // Without its kind, a waiting Run accepts a pause, a cancel, and a steering input. The Run of
    // the author is complete, so its Conversation accepts a steering input of no Run.
    let background = "ffffffff-ffff-7fff-bfff-000000007710".to_owned();
    background_run(&admin, author, &background, "waiting").await;
    background_records(&admin, author, &background).await;
    let pause = issued(
        &store,
        &scope,
        0x771a,
        &PAUSE_AGENT_RUN,
        PauseAgentRunInput {
            run_id: background.clone(),
        },
    )
    .await;
    let cancel = issued(
        &store,
        &scope,
        0x771b,
        &CANCEL_AGENT_RUN,
        CancelAgentRunInput {
            run_id: background.clone(),
        },
    )
    .await;
    let steer = issued(
        &store,
        &scope,
        0x771c,
        &STEER_AGENT_RUN,
        SteerAgentRunInput {
            run_id: background.clone(),
            conversation_id: conversation_id.clone(),
            author_message: "Keep the voice.".to_owned(),
            selection_references: None,
        },
    )
    .await;
    let missing = |refused: bool| match refused {
        true => "missing_run",
        false => "settled",
    };
    let pause_refused = matches!(
        settle_project_command(&store, &pause.envelope, &pause.input).await,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            AgentRunControlRefusal::MissingRun
        ))
    );
    let cancel_refused = matches!(
        settle_project_command(&store, &cancel.envelope, &cancel.input).await,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            AgentRunControlRefusal::MissingRun
        ))
    );
    let steer_refused = matches!(
        settle_project_command(&store, &steer.envelope, &steer.input).await,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            AgentRunControlRefusal::MissingRun
        ))
    );
    let mut rows = Vec::new();
    for issued in [
        &pause.envelope.challenge_binding.idempotency_key,
        &cancel.envelope.challenge_binding.idempotency_key,
        &steer.envelope.challenge_binding.idempotency_key,
    ] {
        rows.push(request_rows(&admin, issued).await);
    }
    let attempt = admin
        .query_one(
            "SELECT model_attempt_id::text FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid",
            &[&background],
        )
        .await
        .unwrap()
        .get::<_, String>(/*idx*/ 0);
    let reads = [
        AgentRunReadSelection::Current,
        AgentRunReadSelection::ModelAttempt(attempt),
    ];
    let mut background_reads = Vec::new();
    for selection in &reads {
        let read = store.read_agent_run(&scope, &background, selection).await;
        background_reads.push(read.unwrap().is_some());
    }
    let author_read = store
        .read_agent_run(&scope, author, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .is_some();
    let sources = [&background, author].map(|run_id| store.list_research_sources(&scope, run_id));
    let [background_sources, author_sources] = sources;
    let sources = (
        background_sources.await.unwrap().is_some(),
        author_sources.await.unwrap().is_some(),
    );
    let list = store.read_project_conversations(&scope, 50).await.unwrap();
    let listed: Vec<String> = list
        .unwrap()
        .into_iter()
        .map(|item| item.conversation_id)
        .collect();
    let read = store
        .read_project_conversation(&scope, conversation_id, 50)
        .await
        .unwrap()
        .unwrap();
    let conversation_runs: Vec<String> = read
        .newest_first
        .into_iter()
        .map(|run| run.run_id)
        .collect();
    // A read in the Scope of another Project gets no background Run of this Project.
    let other = seed_project(&store, "771e").await;
    let other_read = store
        .read_agent_run(&other, &background, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .is_some();
    let visible = (
        visible_rows(&store, &scope, &background).await,
        visible_rows(&store, &other, &background).await,
    );

    assert_eq!(
        (
            [pause_refused, cancel_refused, steer_refused].map(missing),
            rows,
            activity_and_receipts(&admin, &background).await,
            background_reads,
            author_read,
            sources,
            listed,
            conversation_runs,
            other_read,
            visible,
        ),
        (
            ["missing_run"; 3],
            vec![[0, 0, 0, 1]; 3],
            [0, 0],
            vec![false, false],
            true,
            (false, true),
            vec![conversation_id.clone()],
            vec![author.clone()],
            false,
            (1, 0),
        )
    );
}

/// The Continuation Binding that the first decision request of the Run names as its prior
/// response, and the binding that the last decision response of the Run `previous` produced.
async fn prior_response(admin: &Client, run_id: &str, previous: &str) -> [Option<String>; 2] {
    let row = admin
        .query_one(
            "SELECT (SELECT payload #>> '{continuation,prior_continuation_binding_id}'
                       FROM storyos.model_attempts
                      WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                      ORDER BY decision_position LIMIT 1),
                    (SELECT continuation_binding_id::text FROM storyos.model_attempts
                      WHERE run_id = $2::text::uuid AND attempt_role = 'decision'
                      ORDER BY decision_position DESC LIMIT 1)",
            &[&run_id, &previous],
        )
        .await
        .unwrap();
    [row.get(/*idx*/ 0), row.get(/*idx*/ 1)]
}

/// The earlier author messages that the later request of the compaction of the Run records.
async fn earlier_messages(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT (payload #> '{operation_requirement,earlier_author_messages}')::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'later_request'",
        &[&run_id],
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_background_run_changes_nothing_for_a_project_conversation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "774").await;
    complete(&store, &admin, &scope, &first).await;
    let conversation_id = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&first],
        )
        .await
        .unwrap()
        .get::<_, String>(/*idx*/ 0);
    let conversation = ConversationSelection::Existing {
        conversation_id: conversation_id.clone(),
    };
    // The identity of the background Run is after each Run of the author, so a read of the last
    // Run that took it would get it.
    let background = "ffffffff-ffff-7fff-bfff-000000007740";
    background_run(&admin, &first, background, "queued").await;
    let admit = async |prefix: &str, message: &str| {
        let destination = DeploymentDestination::HostFakeToolLoop;
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefix,
            destination,
            message,
            conversation.clone(),
            None,
        )
        .await;
        complete(&store, &admin, &scope, &run_id).await;
        run_id
    };
    // The live background Run does not make the Conversation busy.
    let second = admit("775", "Make Mara older.").await;
    // A completed background Run with a settled response gives no prior response and no earlier
    // author message.
    admin
        .execute(
            "UPDATE storyos.agent_runs SET status = 'completed' WHERE run_id = $1::text::uuid",
            &[&background],
        )
        .await
        .unwrap();
    background_records(&admin, &second, background).await;
    let third = admit("776", "Name the inn The Gull.").await;
    let current = format!("{ACTIVE_COMPACTION_REQUEST_PREFIX} Help again.");
    let fourth = admit("777", &current).await;
    let read = store
        .read_project_conversation(&scope, &conversation_id, 50)
        .await
        .unwrap()
        .unwrap();
    let source = |run_id: &str| json!({"run_id": run_id, "message_id": run_id});
    // The first request of each later Run names the last response of the Run before it.
    let [_, first_binding] = prior_response(&admin, &second, &first).await;
    let [_, second_binding] = prior_response(&admin, &third, &second).await;
    let [first_binding, second_binding] =
        [first_binding, second_binding].map(|binding| Some(binding.expect("a response")));

    assert_eq!(
        (
            prior_response(&admin, &second, &first).await,
            prior_response(&admin, &third, &second).await,
            earlier_messages(&admin, &fourth).await,
            read.newest_first
                .into_iter()
                .map(|run| run.run_id)
                .collect::<Vec<_>>(),
        ),
        (
            [first_binding.clone(), first_binding],
            [second_binding.clone(), second_binding],
            json!([source(&first), source(&second), source(&third)]),
            vec![fourth, third, second, first],
        )
    );
}
