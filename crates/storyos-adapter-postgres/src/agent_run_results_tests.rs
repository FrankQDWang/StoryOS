//! The Memory writes of a Run in its results (ADR 0091 section 6, ADR 0094 section 5): the order,
//! a refused call, a version of background work, a read of one Model Attempt, a cancel after
//! a settled write, and a pass that flags an entry of a Run. Test prefixes `a27` and `a28`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_contracts::AgentRunResultInspect;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::{
    AuthorAction, AuthorAt, Moment, cancellation, claim_duties, queued, settle,
};
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::memory_capture::tests::project;
use crate::memory_consolidation::tests::{capture, run_pass, scripted, settled};
use crate::memory_entries::tests::write_versions;

/// The fake script line of one `write_memory` call.
fn script(action: &str, arguments: &Value) -> String {
    format!("SCRIPT:write_memory {action} {arguments}")
}

/// Each result of the Run read of `run_id` as one value: the action and the title of a Memory
/// write, the kind of each other result.
async fn results(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> Value {
    let run = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let results = run.results.unwrap().into_iter().map(|result| match result {
        AgentRunResultInspect::MemoryWrite { action, title, .. } => json!([action, title]),
        AgentRunResultInspect::Answer { .. } => json!("answer"),
        other => json!(format!("{other:?}")),
    });
    json!(results.collect::<Vec<_>>())
}

/// The state of each `write_memory` call of the Run, in call order.
async fn write_states(admin: &Client, run_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT state FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'write_memory'
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_settled_write_is_a_result_at_the_position_of_its_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "fc1").await;
    complete(&store, &admin, &scope, &first).await;
    // Background work wrote the entry 1. Its version names no ToolCall.
    write_versions(
        &admin,
        &scope,
        &[(1, 1, 1, "current", "other", "旧雨", "雨。")],
    )
    .await;
    let message = [
        script(
            "create",
            &json!({"type": "foreshadowing", "title": "铜铃",
                                 "description": "铃声。", "text": "铜铃会再响。"}),
        ),
        script("change", &json!({"entry": 2, "title": "铜铃声"})),
        script("change", &json!({"entry": 9, "title": "无"})),
        script("delete", &json!({"entry": 1})),
    ]
    .join("\n");
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "fc2", &message, new.clone()).await;
    complete(&store, &admin, &scope, &run_id).await;
    let plain = admitted(&store, &scope, &chapter_id, "fc3", "Help.", new).await;
    complete(&store, &admin, &scope, &plain).await;
    let run = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let attempt_id = run.model.as_ref().unwrap().model_attempt_id.clone();
    let attempt = AgentRunReadSelection::ModelAttempt(attempt_id);
    let attempt = store
        .read_agent_run(&scope, &run_id, &attempt)
        .await
        .unwrap();

    assert_eq!(
        write_states(&admin, &run_id).await,
        ["settled", "settled", "refused", "settled"]
    );
    // The refused call gives no result, and the version of background work is no result of
    // either Run.
    assert_eq!(
        (
            results(&store, &scope, &run_id).await,
            results(&store, &scope, &plain).await
        ),
        (
            json!([
                ["create", "铜铃"],
                ["change", "铜铃声"],
                ["delete", "旧雨"],
                "answer"
            ]),
            json!(["answer"])
        )
    );
    // The public decision stays the final answer, and a read of one Model Attempt has no results.
    assert!(matches!(
        run.decision,
        AgentRunDecisionInspect::Advisory { .. }
    ));
    assert_eq!(attempt.unwrap().results, None);
}

/// The author cancels the Run after its `create` settled: the entry is stored, so the Run read
/// gives the write.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_cancelled_after_a_settled_write_gives_the_write() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "fc4").await;
    complete(&store, &admin, &scope, &first).await;
    let create = json!({"type": "character", "title": "林舟", "description": "船夫。",
                        "text": "他不说话。"});
    let new = ConversationSelection::New;
    let message = script("create", &create);
    let run_id = admitted(&store, &scope, &chapter_id, "fc5", &message, new).await;
    let cancel = cancellation(&store, &scope, 0xfca0, &run_id).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(2),
        AuthorAction::Cancel(Box::new(cancel)),
    );
    let _cancelled = settle(&store, &admin, &scope, &run_id, &author).await;
    // A new claim does the cancellation duties of the request that was out, so the Run leaves no
    // work for the Worker of a later test.
    let claim = claim_duties(&admin, &scope, &run_id).await.unwrap();
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let status: String = admin
        .query_one(
            "SELECT status || ' ' || wakeup_pending FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (status.as_str(), results(&store, &scope, &run_id).await),
        ("cancelled false", json!([["create", "林舟"]]))
    );
}

/// A pass flags the entry that a Run created. A pass can only flag an entry of the author, and its
/// version keeps the source of the call, but it is background work, so the Run gives only its own
/// write (#1692).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_that_flags_an_entry_of_a_run_gives_the_run_no_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "a27").await;
    let message = script(
        "create",
        &json!({"type": "character", "title": "林舟", "description": "船夫。", "text": "渡口。"}),
    );
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "a28", &message, new).await;
    complete(&store, &admin, &scope, &run_id).await;
    let flag = json!({
        "action": "flag_conflict", "entry": 1, "entries": null, "type": null, "title": null,
        "description": null, "text": null, "observations": [1], "reason": "铃已碎。"
    });
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([scripted("脚本", &[flag])]),
    )
    .await;
    let pass = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            settled(&admin, &pass.run_id).await[3].clone(),
            results(&store, &scope, &run_id).await
        ),
        (
            json!([["applied", null, 1]]),
            json!([["create", "林舟"], "answer"])
        )
    );
}
