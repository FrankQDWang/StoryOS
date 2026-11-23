//! The reads of the Subruns of a Run: the field `subruns` of the Run read and the record of one
//! Subrun (ADR 0103 section 3A, items 1 and 2 of #1836). Test prefix leads `91` and `92`.

use serde_json::json;
use storyos_adapter_fake_destination::{FakeDestination, START_SUBRUN_SCRIPT, SUBRUN_SCRIPT_TITLE};
use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, ContractFaultObserver, ContractFaultPoint,
    NoContractFaults, PauseAgentRunInput, ProjectScope, SubrunEntry, SubrunRecord,
    complete_agent_run,
};
use storyos_core::{SubrunState, SubrunStep};
use tokio_postgres::Client;

use super::control_tests::{Command, commanded};
use super::tests::{parent, plan_parent, scripted, subruns};
use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_read_chapter::project;
use crate::shared_tests::agent_run_retrieval::adapter;
use crate::shared_tests::agent_run_steering::cancellation;
use crate::shared_tests::agent_run_tool_exposure::{ADVISORY, finish_then_advisory, sse};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_agent_run::PAUSE_AGENT_RUN;
use crate::shared_tests::command_sequence_support::{issued, stores};
use crate::shared_tests::memory_work_schedule::claim_due;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run};

/// The Blocks of Chapter A of the Project of the first test.
const SENTENCES: [&str; 2] = ["灯在渡口亮着。", "林舟没有上船。"];

/// The states of the entries `subruns` of the Run read of `run_id`.
async fn states(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> Option<Vec<SubrunState>> {
    let record = store.read_agent_run(scope, run_id, &AgentRunReadSelection::Current);
    let entries = record.await.unwrap().unwrap().subruns;
    entries.map(|entries| entries.into_iter().map(|entry| entry.state).collect())
}

/// The Contract Fault Point observer that keeps, at each dispatch claim, the states of the field
/// `subruns` of the Run read of the parent Run.
struct SubrunsAtDispatch<'a> {
    store: &'a PostgresProjectReader,
    scope: &'a ProjectScope,
    run_id: &'a str,
    seen: tokio::sync::Mutex<Vec<Option<Vec<SubrunState>>>>,
}

impl ContractFaultObserver for SubrunsAtDispatch<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        if point == ContractFaultPoint::DispatchClaimed {
            let seen = states(self.store, self.scope, self.run_id).await;
            self.seen.lock().await.push(seen);
        }
    }
}

/// The record of the Subrun `subrun_id` in `scope`.
async fn record(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    subrun_id: &str,
) -> Option<SubrunRecord> {
    store.read_subrun(scope, subrun_id).await.unwrap()
}

/// The Chapter of the Run `run_id`.
async fn chapter_of(admin: &Client, run_id: &str) -> String {
    (admin.query_one(
        "SELECT chapter_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
        &[&run_id],
    ))
    .await
    .unwrap()
    .get(/*idx*/ 0)
}

/// Items 1 and 2 of #1836 with the scripted message of S9-02: while the Subrun works, the Run
/// read has one entry `working`, and after its end the entry is `completed`. The record of the
/// Subrun has the title and the brief of the call, one `chapter_read` entry, the step count, and
/// the final answer. The stored result of the read has the text of Chapter A, and the record has
/// none of it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_run_read_gives_the_working_subrun_and_the_record_gives_its_end() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _, _) = project(&store, &admin, "920", 0x91e0, &SENTENCES).await;
    let run_id = Admission {
        message: START_SUBRUN_SCRIPT,
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_a, "921")
    .await;
    let observer = SubrunsAtDispatch {
        store: &store,
        scope: &scope,
        run_id: &run_id,
        seen: tokio::sync::Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &FakeDestination, &observer, &RuntimeClock, &claim)
        .await
        .unwrap();
    let subrun = subruns(&admin, &run_id).await.remove(0);
    let read = store.read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current);
    let entries = read.await.unwrap().unwrap().subruns;
    let record = record(&store, &scope, &subrun).await;
    let shown = format!("{record:?}");
    let stored_read: String = admin
        .query_one(
            "SELECT result_text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'read_chapter'",
            &[&subrun],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let working = Some(vec![SubrunState::Working]);
    let completed = Some(vec![SubrunState::Completed]);

    assert_eq!(
        (
            observer.seen.into_inner(),
            entries,
            record,
            SENTENCES.map(|sentence| (stored_read.contains(sentence), shown.contains(sentence))),
        ),
        (
            // The first claim of the parent, the three claims of the Subrun, and the two later
            // claims of the parent.
            vec![
                None,
                working.clone(),
                working.clone(),
                working,
                completed.clone(),
                completed,
            ],
            Some(vec![SubrunEntry {
                subrun_id: subrun.clone(),
                title: SUBRUN_SCRIPT_TITLE.to_owned(),
                state: SubrunState::Completed,
            }]),
            Some(SubrunRecord {
                title: SUBRUN_SCRIPT_TITLE.to_owned(),
                // Chapter A is the first Chapter of the Chapter list.
                brief: "SCRIPT:read_chapter 1".to_owned(),
                state: SubrunState::Completed,
                // The script of the Subrun: the read, `finish`, and the final answer.
                step_count: 3,
                steps: vec![SubrunStep::ChapterRead {
                    chapter_id: chapter_a,
                    title: "Chapter A".to_owned(),
                }],
                // The fake destination answers the brief of the Subrun with its advice text.
                answer: Some(storyos_core::ADVISORY_TEXT.to_owned()),
            }),
            [(true, false); 2],
        )
    );
}

/// Items 1 and 2 of #1836: a Subrun that ends with no final answer is `no_result`, the Subrun of
/// a cancelled parent is `stopped`, and the Subrun of a paused parent is `working`. Only a
/// `completed` Subrun has an answer.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_reads_give_no_result_stopped_and_working_for_a_paused_parent() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let empty = r#"SCRIPT:refuse start_subrun {"title":"读","brief":"SCRIPT:empty_final_answer"}"#;
    let (ended, ended_run) = scripted(&store, &admin, "922", "923", empty).await;
    complete(&store, &admin, &ended, &ended_run).await;
    let (cancelled, cancelled_run) =
        scripted(&store, &admin, "924", "925", START_SUBRUN_SCRIPT).await;
    let cancel = Command::Cancel(cancellation(&store, &cancelled, 0x92f0, &cancelled_run).await);
    assert!(commanded(&store, &admin, (&cancelled, &cancelled_run), cancel).await);
    let (paused, paused_run) = scripted(&store, &admin, "926", "927", START_SUBRUN_SCRIPT).await;
    let input = PauseAgentRunInput {
        run_id: paused_run.clone(),
    };
    let pause = Command::Pause(issued(&store, &paused, 0x92f1, &PAUSE_AGENT_RUN, input).await);
    assert!(commanded(&store, &admin, (&paused, &paused_run), pause).await);
    let mut reads = Vec::new();
    for (scope, run_id) in [
        (&ended, &ended_run),
        (&cancelled, &cancelled_run),
        (&paused, &paused_run),
    ] {
        let subrun = subruns(&admin, run_id).await.remove(0);
        let record = record(&store, scope, &subrun).await.unwrap();
        let state = states(&store, scope, run_id).await;
        reads.push((state, record.state, record.answer));
    }

    assert_eq!(
        reads,
        [
            (
                Some(vec![SubrunState::NoResult]),
                SubrunState::NoResult,
                None
            ),
            (Some(vec![SubrunState::Stopped]), SubrunState::Stopped, None),
            (Some(vec![SubrunState::Working]), SubrunState::Working, None),
        ]
    );
}

/// Item 1 of #1836: a Run with no Subrun Request has no field `subruns`, and a refused
/// `start_subrun` call gives no entry.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_with_no_subrun_request_has_no_field_subruns() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (reader, reader_run) = parent(&store, &admin, "928", "929").await;
    let title = r#"SCRIPT:refuse start_subrun {"title":" ","brief":"读第一章"}"#;
    let (refused, refused_run) = scripted(&store, &admin, "92a", "92b", title).await;
    complete(&store, &admin, &refused, &refused_run).await;

    assert_eq!(
        (
            states(&store, &reader, &reader_run).await,
            states(&store, &refused, &refused_run).await,
        ),
        (None, None)
    );
}

/// Item 2 of #1836: `getSubrun` answers only for a Subrun Request of a Run of the author of the
/// Project. The identity of a Run of the author, of a background Run, and of a Subrun of another
/// Project each answer as an AgentRun that does not exist.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_record_of_a_subrun_answers_only_for_a_subrun_of_the_project() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, run_id) = scripted(&store, &admin, "92c", "92d", START_SUBRUN_SCRIPT).await;
    complete(&store, &admin, &scope, &run_id).await;
    let subrun = subruns(&admin, &run_id).await.remove(0);
    let conversation: String = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    claim_due(&store, &admin, &scope, &conversation).await;
    let background: String = admin
        .query_one(
            "SELECT run_id::text FROM storyos.agent_runs
              WHERE project_id = $1::text::uuid AND run_kind = 'memory_capture'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let (other, _) = parent(&store, &admin, "92e", "92f").await;

    assert_eq!(
        (
            record(&store, &scope, &subrun).await.is_some(),
            record(&store, &scope, &run_id).await,
            record(&store, &scope, &background).await,
            record(&store, &other, &subrun).await,
        ),
        (true, None, None, None)
    );
}

/// Item 2 of #1836: on the Agent Plan Registration, the text that the Subrun writes beside its
/// function call is a `text` entry before the entry of the call. The final answer is not an
/// entry. The model serves the call of the parent, one response of the Subrun with a text and a
/// `read_chapter` call, the `finish` call and the answer of the Subrun, and the end of the
/// parent.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_text_beside_the_calls_of_a_subrun_is_an_entry_before_them() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let text = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": "先读第一章。"}]});
    let call = json!({"id": "fc_read", "type": "function_call", "status": "completed",
        "call_id": "call_read", "name": "read_chapter",
        "arguments": r#"{"chapter":1,"first_block":null,"last_block":null}"#});
    let read = sse(&[
        json!({"type": "response.created",
               "response": {"id": "resp_910_read", "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": text}),
        json!({"type": "response.output_item.done", "output_index": 1, "item": call}),
        json!({"type": "response.completed", "response": {"id": "resp_910_read",
               "status": "completed", "output": [text, call],
               "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ]);
    let mut subrun = vec![read];
    subrun.extend(finish_then_advisory("resp_910_subrun"));
    let (scope, run_id, served) = plan_parent(&store, &admin, "910", subrun).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    (complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim))
        .await
        .unwrap();
    let subrun = subruns(&admin, &run_id).await.remove(0);

    assert_eq!(
        (
            record(&store, &scope, &subrun).await,
            served.await.unwrap().len(),
        ),
        (
            Some(SubrunRecord {
                title: "读".to_owned(),
                brief: "读第一章。".to_owned(),
                state: SubrunState::Completed,
                step_count: 3,
                steps: vec![
                    SubrunStep::Text {
                        text: "先读第一章。".to_owned()
                    },
                    SubrunStep::ChapterRead {
                        chapter_id: chapter_of(&admin, &run_id).await,
                        title: "Chapter A".to_owned(),
                    },
                ],
                answer: Some(ADVISORY.to_owned()),
            }),
            6,
        )
    );
}
