//! An intent pass in the store (ADR 0101 section 4): the start after an applied consolidation
//! pass, the version with the result, the context of one file item, the refused calls, a pass
//! with no settled call, the order of two entries, the entries of the author, a new text after a
//! result, the archive, and a refused start. Each test completes each Run that it queued.

use serde_json::{Value, json};
use storyos_application::{
    MemoryCaptureSource, MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource, ProjectScope,
};
use storyos_core::IntentPassSkip;
use tokio_postgres::Client;

use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{NoContractFaults, complete_agent_run};

use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_background_run::claim_in_project;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::{park_queued_author_runs, project};
use crate::shared_tests::memory_consolidation::{capture, delete, run_pass, scripted, settled};
use crate::shared_tests::memory_entries::{archived_rows, write_versions};
use crate::shared_tests::model_gateway::RuntimeClock;

/// A consolidation create of the character `title` with `text`.
fn character(title: &str, text: &str) -> Value {
    json!({
        "action": "create", "entry": null, "entries": null, "type": "character", "title": title,
        "description": "船夫。", "text": text, "observations": [1], "reason": null
    })
}

/// A consolidation update of the entry `entry` to the title `title` and the text `text`.
fn change(entry: u64, title: &str, text: &str) -> Value {
    json!({
        "action": "update", "entry": entry, "entries": null, "type": null, "title": title,
        "description": "船夫。", "text": text, "observations": [1], "reason": null
    })
}

/// The text `text` with one fake script line for each call of `calls`, in order.
fn with_calls(text: &str, calls: &[Value]) -> String {
    let lines = calls.iter().map(|call| format!("\nSCRIPT:intent {call}"));
    format!("{text}{}", lines.collect::<String>())
}

/// Settles one capture and one consolidation pass of `operations` in the Project of `scope`.
async fn consolidate(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    operations: &[Value],
) -> String {
    let observation = json!([scripted("脚本", operations)]);
    capture(store, admin, scope, source, observation).await;
    run_pass(store, admin, scope).await.run_id
}

/// Makes each due row of the schedule not due, except the intent pass row of the Project, so that
/// the claim can take only that row. The due time of that row stays as the settlement wrote it.
async fn claim(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
) -> MemoryWorkOutcome {
    admin
        .execute(
            "UPDATE storyos.memory_work_schedules SET due_at = NULL
              WHERE due_at IS NOT NULL
                AND NOT (project_id = $1::text::uuid AND work_kind = 'intent_pass')",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let claimed = store.claim_due_memory_work().await.unwrap().unwrap();
    assert_eq!(
        (&claimed.project_scope, &claimed.source),
        (scope, &MemoryWorkSource::ProjectIntents)
    );
    claimed.outcome
}

/// Makes the intent pass row of the Project due now with no settlement. A test uses it only where
/// no settlement makes the row due: before a claim that must start no pass, and after a refused
/// start.
async fn make_due(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, 'intent_pass', $2::text::uuid,
                     clock_timestamp() - interval '1 second')
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// Claims the intent pass row, then completes the Run of the pass that it starts. Returns the Run.
async fn run_intent(store: &PostgresProjectReader, admin: &Client, scope: &ProjectScope) -> String {
    let MemoryWorkOutcome::IntentPassStarted(started) = claim(store, admin, scope).await else {
        panic!("the claim must start an intent pass");
    };
    finish(store, admin, scope, &started.run_id).await;
    started.run_id
}

/// Completes the intent pass Run `run_id` through the claim of the Worker, one RunStep for each
/// claim.
async fn finish(store: &PostgresProjectReader, admin: &Client, scope: &ProjectScope, run_id: &str) {
    for _ in 0..=storyos_core::INTENT_PASS_MAX_RUN_STEPS {
        let claimed = claim_in_project(store, scope).await.unwrap();
        assert_eq!(claimed.run_id, run_id);
        complete_agent_run(
            store,
            &FakeDestination,
            &NoContractFaults,
            &RuntimeClock,
            &claimed,
        )
        .await
        .unwrap();
        let status = json_of(
            admin,
            "SELECT to_jsonb(status)::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await;
        if status != json!("queued") {
            return;
        }
    }
    panic!("the intent pass {run_id} did not end");
}

/// Whether the intent pass row of the Project is due.
async fn intent_due(admin: &Client, scope: &ProjectScope) -> Value {
    json_of(
        admin,
        "SELECT to_jsonb(due_at IS NOT NULL)::text FROM storyos.memory_work_schedules
          WHERE project_id = $1::text::uuid AND work_kind = 'intent_pass'",
        &[&scope.project_id.as_ref()],
    )
    .await
}

/// Each pass of the Project in start order: the entry number, the outcome, the failure reason,
/// the versions that it read, the Run kind and status, the number of its RunSteps, and the state
/// and refusal of each call.
async fn passes(admin: &Client, scope: &ProjectScope) -> Value {
    json_of(
        admin,
        "SELECT COALESCE(jsonb_agg(jsonb_build_array(
                      (SELECT DISTINCT entry_number FROM storyos.memory_entry_versions AS version
                        WHERE version.memory_entry_id = pass.memory_entry_id),
                      pass.outcome, pass.failure_reason, pass.entry_version, pass.author_version,
                      run.run_kind, run.status,
                      (SELECT count(*) FROM storyos.context_assembly_manifests AS manifest
                        WHERE manifest.run_id = pass.run_id
                          AND manifest.manifest_role = 'decision'),
                      (SELECT COALESCE(jsonb_agg(jsonb_build_array(call.state, call.refusal)
                                                 ORDER BY call.decision_position), '[]')
                         FROM storyos.tool_calls AS call WHERE call.run_id = pass.run_id))
                      ORDER BY pass.started_at, pass.pass_id), '[]')::text
           FROM storyos.intent_passes AS pass JOIN storyos.agent_runs AS run USING (run_id)
          WHERE pass.project_id = $1::text::uuid",
        &[&scope.project_id.as_ref()],
    )
    .await
}

/// Each version of the entry `number` in version order: the version, the cause kind, the intent,
/// and the questions.
async fn intents(admin: &Client, scope: &ProjectScope, number: u64) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(version, cause ->> 'kind', intent, intent_questions)
                          ORDER BY version)::text
           FROM storyos.memory_entry_versions
          WHERE project_id = $1::text::uuid AND entry_number = $2::text::numeric",
        &[&scope.project_id.as_ref(), &number.to_string()],
    )
    .await
}

async fn memory_revision(admin: &Client, scope: &ProjectScope) -> Value {
    json_of(
        admin,
        "SELECT to_jsonb(memory_revision)::text FROM storyos.project_memory_heads
          WHERE project_id = $1::text::uuid",
        &[&scope.project_id.as_ref()],
    )
    .await
}

/// The time of the last change of the entry `number` in `listMemoryEntries` and in a read.
async fn changed_at(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
) -> [String; 2] {
    let listed = storyos_application::list_memory_entries(store, scope).await;
    let listed = listed
        .unwrap()
        .unwrap()
        .into_iter()
        .find(|entry| entry.number == 2);
    let read = crate::memory_entries::read_current_entry(admin, scope, 2).await;
    [
        listed.unwrap().changed_at,
        read.unwrap().unwrap().changed_at,
    ]
}

/// A Project with a Chapter that has text, a second empty Chapter, an entry of the author of
/// another type, and the source of its completed Run of the author.
async fn source_project(
    store: &PostgresProjectReader,
    admin: &Client,
    lead: &str,
    suffix: u16,
) -> (ProjectScope, String, MemoryCaptureSource) {
    park_queued_author_runs(admin).await;
    let (scope, chapter, _, _) = crate::shared_tests::agent_run_read_chapter::project(
        store,
        admin,
        lead,
        suffix,
        &["林舟站在渡口。"],
    )
    .await;
    let run = admin
        .query_one(
            "SELECT conversation_id::text, run_id::text FROM storyos.agent_runs
              WHERE project_id = $1::text::uuid AND run_kind = 'author'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let source = MemoryCaptureSource::Conversation {
        conversation_id: run.get(/*idx*/ 0),
        first_run_id: run.get(/*idx*/ 1),
        last_run_id: run.get(/*idx*/ 1),
    };
    write_versions(
        admin,
        &scope,
        &[(1, 1, 1, "current", "other", "渡口", "河边")],
    )
    .await;
    (scope, chapter, source)
}

/// ADR 0101 section 4: an applied consolidation pass that writes a character entry makes the row
/// due, and the claim starts one pass with no author action. The pass completes on the fake, and
/// it writes one version with the intent, the questions, and the Chapter at the next Memory
/// revision. The version copies each other field, and the time of the last change stays. The
/// context has only the file item. The archive has the pass and the new columns. No second pass
/// starts while the versions do not change.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_pass_starts_one_intent_pass_that_records_a_version_with_the_intent() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter, source) = source_project(&store, &admin, "4c0", 0x4ca0).await;
    let create = character("林舟", "林舟是渡口的船夫。");
    consolidate(&store, &admin, &scope, &source, &[create]).await;
    let (due, revision) = (
        intent_due(&admin, &scope).await,
        memory_revision(&admin, &scope).await,
    );
    let before = changed_at(&store, &admin, &scope).await;
    let run_id = run_intent(&store, &admin, &scope).await;
    let fields = "SELECT jsonb_agg(jsonb_build_array(state, entry_type, origin, title, description,
                      text, source_references, intent_chapter_id::text) ORDER BY version)::text
                    FROM storyos.memory_entry_versions
                   WHERE project_id = $1::text::uuid AND entry_number = 2";
    let fields = json_of(&admin, fields, &[&scope.project_id.as_ref()]).await;
    let context = json_of(
        &admin,
        "SELECT jsonb_agg(jsonb_build_array(item ->> 'source_class', item ->> 'content'))::text
           FROM storyos.context_assembly_manifests AS manifest,
                jsonb_array_elements(manifest.payload -> 'selected') AS item
          WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'",
        &[&run_id],
    )
    .await;
    let [archived, versions] = archived_rows(
        &store,
        &scope,
        "4c",
        ["intent_passes", "memory_entry_versions"],
    )
    .await;
    let archived_version = versions
        .iter()
        .find(|row| !row["intent"].is_null())
        .unwrap();
    let archived = (
        archived
            .iter()
            .map(|row| row["outcome"].clone())
            .collect::<Vec<_>>(),
        &archived_version["intent_questions"],
        archived_version["intent_chapter_id"] == chapter.as_str(),
        archived_version["intent_author_entry_id"].is_null(),
    );

    assert_eq!(
        fields[0].as_array().unwrap()[..7],
        fields[1].as_array().unwrap()[..7]
    );
    assert_eq!(
        (
            (due, revision, memory_revision(&admin, &scope).await),
            passes(&admin, &scope).await,
            intents(&admin, &scope, 2).await,
            (&fields[0][7], &fields[1][7]),
            (before.clone(), changed_at(&store, &admin, &scope).await),
            context,
            archived,
            {
                make_due(&admin, &scope).await;
                claim(&store, &admin, &scope).await
            }
        ),
        (
            (json!(true), json!(2), json!(3)),
            json!([[
                2,
                "recorded",
                null,
                1,
                null,
                "intent_pass",
                "completed",
                1,
                [["settled", null]]
            ]]),
            json!([
                [1, "consolidation", null, null],
                [
                    2,
                    "intent_pass",
                    "我明天一早去渡口，看看那条船还在不在。",
                    ["我以前去过河对岸吗？"]
                ]
            ]),
            (&Value::Null, &json!(chapter)),
            (before.clone(), before),
            json!([[
                "character_file",
                "{\"name\":\"林舟\"}\n\nFrom the manuscript:\n林舟是渡口的船夫。"
            ]]),
            (
                vec![json!("recorded")],
                &json!(["我以前去过河对岸吗？"]),
                true,
                true
            ),
            MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::NoEntryNeedsPass)
        )
    );
}

/// The Host refuses each call outside the limits with the label of Core, and a refused call
/// writes no version. A correct second call records the result. A pass with two refused calls,
/// and a pass with no call, fail with `no_intent_call`. Three entries run one after the other in
/// the order of the numbers, and each settlement makes the row due while an entry needs a pass.
/// A refused start writes no pass and gives no error.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn refused_calls_write_no_version_and_the_passes_run_in_entry_order() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = source_project(&store, &admin, "4c1", 0x4cb0).await;
    let call =
        |intent: &str, questions: &[&str]| json!({ "intent": intent, "questions": questions });
    let line_break = call("我去渡口。", &["我会\n游泳吗？"]);
    let correct = call("我去渡口。", &[]);
    let long = call(&"长".repeat(151), &[]);
    let four = call("我去渡口。", &["一？", "二？", "三？", "四？"]);
    let operations = [
        character("林舟", &with_calls("林舟是船夫。", &[line_break, correct])),
        character("阿青", &with_calls("阿青是船夫。", &[long, four])),
        character("老周", "老周是船夫。\nSCRIPT:intent-no-call"),
    ];
    consolidate(&store, &admin, &scope, &source, &operations).await;
    let project_id = scope.project_id.as_ref();
    let policy = "UPDATE storyos.project_policy_revisions SET availability = $2
                   WHERE project_id = $1::text::uuid";
    admin
        .execute(policy, &[&project_id, &"unavailable"])
        .await
        .unwrap();
    let refused = (
        claim(&store, &admin, &scope).await,
        passes(&admin, &scope).await,
    );
    admin
        .execute(policy, &[&project_id, &"available"])
        .await
        .unwrap();
    // The refused start made the row not due, and no settlement makes it due again.
    make_due(&admin, &scope).await;
    let (mut due, mut versions) = (Vec::new(), Vec::new());
    for number in [2, 3, 4] {
        run_intent(&store, &admin, &scope).await;
        due.push(intent_due(&admin, &scope).await);
        versions.push(intents(&admin, &scope, number).await);
    }

    assert_eq!(
        (refused, due, passes(&admin, &scope).await, versions),
        (
            (
                MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::StartRefused),
                json!([])
            ),
            vec![json!(true), json!(true), json!(false)],
            json!([
                [
                    2,
                    "recorded",
                    null,
                    1,
                    null,
                    "intent_pass",
                    "completed",
                    2,
                    [["refused", "question_invalid"], ["settled", null]]
                ],
                [
                    3,
                    "failed",
                    "no_intent_call",
                    1,
                    null,
                    "intent_pass",
                    "completed",
                    2,
                    [
                        ["refused", "intent_too_long"],
                        ["refused", "too_many_questions"]
                    ]
                ],
                [
                    4,
                    "failed",
                    "no_intent_call",
                    1,
                    null,
                    "intent_pass",
                    "completed",
                    1,
                    []
                ]
            ]),
            vec![
                json!([
                    [1, "consolidation", null, null],
                    [2, "intent_pass", "我去渡口。", []]
                ]),
                json!([[1, "consolidation", null, null]]),
                json!([[1, "consolidation", null, null]])
            ]
        )
    );
}

/// A character with only the entry of the author starts no pass. A pass operation on the entry
/// of the author is refused as before, and the pass of the entry of the background work with the
/// same title reads both parts and writes no version of the entry of the author. A new text of the
/// entry after a result is a version with no intent, and the entry needs a pass again.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_entry_of_the_author_gets_no_version_and_a_new_text_needs_a_pass_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "4c2").await;
    write_versions(
        &admin,
        &scope,
        &[(1, 1, 1, "current", "character", "阿青", "怕水")],
    )
    .await;
    make_due(&admin, &scope).await;
    let author_only = claim(&store, &admin, &scope).await;
    let operations = [character("阿青", "阿青是船夫。"), change(1, "阿青", "改。")];
    let first = consolidate(&store, &admin, &scope, &source, &operations).await;
    let first_pass = run_intent(&store, &admin, &scope).await;
    let file = json_of(
        &admin,
        "SELECT to_jsonb(item ->> 'content')::text
           FROM storyos.context_assembly_manifests AS manifest,
                jsonb_array_elements(manifest.payload -> 'selected') AS item
          WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'",
        &[&first_pass],
    )
    .await;
    let new_text = [change(2, "阿青", "阿青换了船。")];
    let changed = consolidate(&store, &admin, &scope, &source, &new_text).await;
    let again = intent_due(&admin, &scope).await;
    run_intent(&store, &admin, &scope).await;
    let fake = "我明天一早去渡口，看看那条船还在不在。";
    let questions = json!(["我以前去过河对岸吗？"]);

    assert_eq!(
        (
            author_only,
            settled(&admin, &first).await[3].clone(),
            file,
            settled(&admin, &changed).await[1].clone(),
            again,
            passes(&admin, &scope).await,
            intents(&admin, &scope, 1).await,
            intents(&admin, &scope, 2).await
        ),
        (
            MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::NoEntryNeedsPass),
            json!([["applied", null, 2], ["refused", "author_entry", null]]),
            json!(
                "{\"name\":\"阿青\"}\n\nTold by the author:\n怕水\n\nFrom the manuscript:\n阿青是船夫。"
            ),
            json!("applied"),
            json!(true),
            json!([
                [
                    2,
                    "recorded",
                    null,
                    1,
                    1,
                    "intent_pass",
                    "completed",
                    1,
                    [["settled", null]]
                ],
                [
                    2,
                    "recorded",
                    null,
                    3,
                    1,
                    "intent_pass",
                    "completed",
                    1,
                    [["settled", null]]
                ]
            ]),
            json!([[1, null, null, null]]),
            json!([
                [1, "consolidation", null, null],
                [2, "intent_pass", fake, questions],
                [3, "consolidation", null, null],
                [4, "intent_pass", fake, questions]
            ])
        )
    );
}

/// A consolidation pass deletes the entry after the start of an intent pass: the valid call
/// settles, the pass fails with `entry_not_current`, the entry gets no intent version, and the
/// next claim starts no pass.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_delete_during_a_pass_fails_the_pass_and_writes_no_version() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "4c3").await;
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    let MemoryWorkOutcome::IntentPassStarted(started) = claim(&store, &admin, &scope).await else {
        panic!("the claim must start an intent pass");
    };
    consolidate(&store, &admin, &scope, &source, &[delete(1)]).await;
    let revision = memory_revision(&admin, &scope).await;
    finish(&store, &admin, &scope, &started.run_id).await;
    let result = "SELECT jsonb_agg(result_text)::text FROM storyos.tool_calls
                   WHERE run_id = $1::text::uuid";

    assert_eq!(
        (
            passes(&admin, &scope).await,
            json_of(&admin, result, &[&started.run_id]).await,
            intents(&admin, &scope, 1).await,
            memory_revision(&admin, &scope).await,
            claim(&store, &admin, &scope).await
        ),
        (
            json!([[
                1,
                "failed",
                "entry_not_current",
                1,
                null,
                "intent_pass",
                "completed",
                1,
                [["settled", null]]
            ]]),
            json!([
                "The Host did not record the intent: the entry is no longer a current character entry of the background work."
            ]),
            json!([
                [1, "consolidation", null, null],
                [2, "consolidation", null, null]
            ]),
            revision,
            MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::NoEntryNeedsPass)
        )
    );
}
