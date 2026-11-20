//! Steering and cancellation of a Run of the tool loop profile at each RunStep position
//! (ADR 0062, ADR 0086, ADR 0091), with the fake model of that profile.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunControlRefusal, AgentRunReadSelection, AgentRunReadStore, ContractFaultPoint,
    ConversationSelection, NoContractFaults, PauseAgentRunInput, ProjectScope, complete_agent_run,
    complete_agent_run_with_tools,
};
use storyos_core::{AgentRunStatus, DeploymentDestination, RESEARCH_TOOL_OFFER};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_propose_edit::{REVISE, opened, propose_edit_calls};
use crate::shared_tests::agent_run_propose_edit_revise::Fixture;
use crate::shared_tests::agent_run_retrieval::destination;
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, SteeringAnswer, cancellation, claim_duties, queued, run_steps,
    settle, steering, step_references, step_requests,
};
use crate::shared_tests::agent_run_tool_dispatch::{client, exa_tools, mcp_replies, point_mcp};
use crate::shared_tests::agent_run_tool_recovery::{Fault, FaultAt};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_agent_run::PAUSE_AGENT_RUN;
use crate::shared_tests::command_sequence_support::issued;
use crate::shared_tests::model_gateway::{
    RuntimeClock, admit_referenced_run, admit_run_in, claim_run, queued_run_offering, stores,
};

/// The status of the Run, and each steering input as its position, its text, and whether a
/// decision Model Attempt read it.
async fn steering_facts(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> Value {
    let record = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let inputs: Vec<Value> = record
        .steering_inputs
        .iter()
        .map(|input| {
            json!([
                input.input_position,
                input.author_message,
                input.model_attempt_id.is_some()
            ])
        })
        .collect();
    json!({"status": format!("{:?}", record.status), "inputs": inputs})
}

const MESSAGE: &str = "Help with this passage.";
const STEERED: &str = "Help with this passage.\nKeep the voice.";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_author_steers_at_each_run_step_position_and_the_next_run_step_reads_the_input() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (required, none) = ("required", "none");
    let call = "call_finish";
    let cases = [
        // While the request of RunStep 1 is out. Its `finish` call counts, but the next RunStep
        // reads the input, so it is an ordinary RunStep.
        (
            "f40",
            0xf600,
            Moment::DuringRequest(1),
            json!([
                [0, "tool_request", MESSAGE, required, []],
                [1, "tool_request", STEERED, required, [call]],
                [2, "advisory", null, none, [call, "call_finish_1"]]
            ]),
            "1",
        ),
        // After the result of a `finish` call that counted, and before the closing request goes
        // out. The closing RunStep sends no request.
        (
            "f41",
            0xf610,
            Moment::BeforeRequest(2),
            json!([
                [0, "tool_request", MESSAGE, required, []],
                [2, "tool_request", STEERED, required, [call]],
                [3, "advisory", null, none, [call, "call_finish_1"]]
            ]),
            "2",
        ),
        // While the closing request is out. Its final answer does not complete the Run. The
        // closing request continues the prior response with the same input, so it sends only
        // the output.
        (
            "f42",
            0xf620,
            Moment::DuringRequest(2),
            json!([
                [0, "tool_request", MESSAGE, required, []],
                [1, "advisory", null, none, [call]],
                [2, "tool_request", STEERED, required, [call]],
                [3, "advisory", null, none, [call, "call_finish_1"]]
            ]),
            "2",
        ),
    ];
    for (prefix, suffix, moment, expected_steps, position) in cases {
        let (scope, _, run_id) = queued(&store, prefix).await;
        let input = steering(&store, &scope, suffix, &run_id, "Keep the voice.", None).await;
        let author = AuthorAt::new(&store, moment, AuthorAction::Steer(vec![input]));
        settle(&store, &admin, &scope, &run_id, &author)
            .await
            .unwrap();

        assert_eq!(
            (
                step_requests(&admin, &run_id).await,
                steering_facts(&store, &scope, &run_id).await,
                author.answers.into_inner().unwrap(),
            ),
            (
                expected_steps,
                json!({"status": format!("{:?}", AgentRunStatus::Completed),
                       "inputs": [[position, "Keep the voice.", true]]}),
                vec![SteeringAnswer::Retained],
            ),
            "{moment:?}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_input_of_a_later_run_step_joins_its_references_up_to_eight() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0xf90, tool_loop.clone()).await;
    let text = |first, last| fixture.chapter_text(first, last);
    let run_references = vec![text(0, 0), text(1, 1), text(2, 2)];
    // The first added reference is a reference of the Run, so five join and the Run holds 8.
    let added = vec![
        text(2, 2),
        text(0, 1),
        text(1, 2),
        text(0, 2),
        text(2, 3),
        text(0, 3),
    ];
    let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
    let message = MESSAGE;
    let references = run_references.clone();
    let run_id = admit_referenced_run(
        &store, scope, chapter_id, "f92", tool_loop, message, references,
    )
    .await;
    let inputs = vec![
        steering(
            &store,
            scope,
            0xf680,
            &run_id,
            "Keep the voice.",
            Some(added.clone()),
        )
        .await,
        steering(
            &store,
            scope,
            0xf681,
            &run_id,
            "And here.",
            Some(vec![text(1, 3)]),
        )
        .await,
    ];
    // While the closing request is out.
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(2),
        AuthorAction::Steer(inputs),
    );
    settle(&store, &admin, scope, &run_id, &author)
        .await
        .unwrap();
    let mut joined = run_references.clone();
    joined.extend(added[1..].iter().cloned());
    let (before, after) = (json!(run_references), json!(joined));

    assert_eq!(
        (
            step_references(&admin, &run_id).await,
            author.answers.into_inner().unwrap(),
        ),
        (
            json!([[0, before], [1, before], [2, after], [3, after]]),
            vec![
                SteeringAnswer::Retained,
                SteeringAnswer::Refused(AgentRunControlRefusal::ReferenceLimit),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn after_a_steering_input_the_next_propose_edit_call_revises_the_moved_candidate() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0xf98, tool_loop.clone()).await;
    let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
    let candidates = fixture.candidates(&admin).await;
    let references = candidates.clone();
    let run_id = admit_referenced_run(
        &store, scope, chapter_id, "f9a", tool_loop, REVISE, references,
    )
    .await;
    // The first RunStep calls `propose_edit` and `finish`. The steering RunStep after it reads
    // one `propose_edit` result, so `propose_again` makes one call alone, and then a `finish`
    // call.
    let message = "Revise it again. SCRIPT:propose_again";
    let input = steering(&store, scope, 0xf690, &run_id, message, None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, scope, &run_id, &author)
        .await
        .unwrap();
    let revisions: Vec<(String, String)> = admin
        .query(
            "SELECT revision.revision_id::text, revision.candidate_text
               FROM storyos.proposal_revisions AS revision
               JOIN storyos.proposals AS proposal USING (owner_user_id, project_id, proposal_id)
              WHERE proposal.source_run_id = $1::text::uuid ORDER BY revision.revision_id",
            &[&fixture.source_run],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .collect();
    // The reference of the Run to all of the candidate text of the revision `index`.
    let reference = |index: usize| {
        let mut reference = json!(candidates[0]);
        reference["revision_id"] = json!(revisions[index].0);
        reference["end_offset"] = json!(revisions[index].1.encode_utf16().count());
        json!([reference])
    };

    // The call after the steering input revises the candidate that the first call made, and
    // each RunStep reads the revision of the call before it.
    assert_eq!(
        (
            propose_edit_calls(&admin, &run_id).await,
            step_references(&admin, &run_id).await,
            run_steps(&admin, &run_id).await["status"].clone(),
        ),
        (
            vec![opened(0), opened(1)],
            json!([
                [0, reference(0)],
                [1, reference(1)],
                [2, reference(2)],
                [3, reference(2)]
            ]),
            json!("completed"),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_author_cancels_at_each_run_step_position_and_no_later_run_step_opens() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let cases = [
        (
            "f43",
            0xf630,
            Moment::DuringRequest(1),
            json!([0]),
            json!([[0, null]]),
        ),
        // After the result of a `finish` call that counted, and before the closing request.
        (
            "f44",
            0xf640,
            Moment::BeforeRequest(2),
            json!([0, 1]),
            json!([[0, "tool_request"]]),
        ),
        // While the closing request is out.
        (
            "f45",
            0xf650,
            Moment::DuringRequest(2),
            json!([0, 1]),
            json!([[0, "tool_request"], [1, null]]),
        ),
    ];
    for (prefix, suffix, moment, requirements, attempts) in cases {
        let (scope, _, run_id) = queued(&store, prefix).await;
        let cancel = Box::new(cancellation(&store, &scope, suffix, &run_id).await);
        let author = AuthorAt::new(&store, moment, AuthorAction::Cancel(cancel));
        let _fenced = settle(&store, &admin, &scope, &run_id, &author).await;
        // A new claim does the cancellation duties of a request that was out.
        if let Some(claim) = claim_duties(&admin, &scope, &run_id).await {
            complete_agent_run(
                &store,
                &FakeDestination,
                &NoContractFaults,
                &RuntimeClock,
                &claim,
            )
            .await
            .unwrap();
        }

        assert_eq!(
            run_steps(&admin, &run_id).await,
            json!({"status": "cancelled", "requirements": requirements, "attempts": attempts}),
            "{moment:?}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancel_fences_the_research_call_of_a_later_run_step_and_no_later_run_step_opens() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let research = Some(&RESEARCH_TOOL_OFFER);
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let (scope, chapter_id, first) =
        queued_run_offering(&store, "f46", tool_loop.clone(), research).await;
    settle(&store, &admin, &scope, &first, &FakeDestination)
        .await
        .unwrap();
    let conversation = ConversationSelection::New;
    let run_id = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "f47",
        tool_loop,
        MESSAGE,
        conversation,
        research,
    )
    .await;
    let (mcp_base, _mcp) = destination(mcp_replies(exa_tools(|_| {}), None)).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    // The steering RunStep asks for research, and the author cancels after its dispatch claim.
    let search = "Search the web: tide tables of 1890";
    let input = steering(&store, &scope, 0xf660, &run_id, search, None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    let cancel = Fault::Cancel {
        store: &store,
        call: Box::new(cancellation(&store, &scope, 0xf661, &run_id).await),
    };
    let observer = FaultAt::new(
        ContractFaultPoint::ToolDispatchClaimed,
        /*nth*/ 1,
        cancel,
    );
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let tools = client();
    let gateway =
        complete_agent_run_with_tools(&store, &author, &tools, &observer, &RuntimeClock, &claim);
    let _fenced = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 120), gateway)
        .await
        .expect("the Run settles");
    let calls: Value = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(decision_position, function_name, state, refusal,
                                                attempt #>> '{fence,cause}')
                              ORDER BY decision_position, call_position)::text
               FROM storyos.tool_calls WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .unwrap();

    assert_eq!(
        (run_steps(&admin, &run_id).await, calls),
        (
            json!({"status": "cancelled", "requirements": [0, 1],
                   "attempts": [[0, "tool_request"], [1, "tool_request"]]}),
            json!([
                [0, "finish", "settled", null, null],
                [1, "web_search", "unknown", null, "run_cancelled"],
                [1, "finish", "refused", "run_cancelled", null]
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_input_that_only_the_last_run_step_can_read_is_conflicted() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // While the request of RunStep 5 is out, RunStep 6 reads the input. While the request of
    // the RunStep before the last or of the last RunStep is out, no ordinary RunStep can read
    // it, so the Run retains nothing and the input becomes the message of the next Run.
    let cases = [
        (
            ["f48", "f49"],
            0xf670,
            5,
            SteeringAnswer::Retained,
            json!([["5", "Keep the voice.", true]]),
        ),
        (
            ["f4c", "f4d"],
            0xf672,
            23,
            SteeringAnswer::Conflicted,
            json!([]),
        ),
        (
            ["f4a", "f4b"],
            0xf671,
            24,
            SteeringAnswer::Conflicted,
            json!([]),
        ),
    ];
    for (prefixes, suffix, request, answer, inputs) in cases {
        let tool_loop = DeploymentDestination::HostFakeToolLoop;
        let (scope, chapter_id, first) = queued(&store, prefixes[0]).await;
        settle(&store, &admin, &scope, &first, &FakeDestination)
            .await
            .unwrap();
        let conversation = ConversationSelection::New;
        let message = "SCRIPT:never_finish";
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefixes[1],
            tool_loop,
            message,
            conversation,
            None,
        )
        .await;
        let input = steering(&store, &scope, suffix, &run_id, "Keep the voice.", None).await;
        let author = AuthorAt::new(
            &store,
            Moment::DuringRequest(request),
            AuthorAction::Steer(vec![input]),
        );
        settle(&store, &admin, &scope, &run_id, &author)
            .await
            .unwrap();
        let choices: Vec<Value> = step_requests(&admin, &run_id)
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step[3].clone())
            .collect();
        let mut expected = vec![json!("required"); 23];
        expected.push(json!("none"));

        assert_eq!(
            (
                choices,
                steering_facts(&store, &scope, &run_id).await,
                author.answers.into_inner().unwrap(),
            ),
            (
                expected,
                json!({"status": format!("{:?}", AgentRunStatus::Completed), "inputs": inputs}),
                vec![answer],
            ),
            "{request}"
        );
    }
}

/// The status and the active position of the Run, the positions of its Operation Requirements,
/// and the author instruction of the requirement at the active position.
async fn run_position(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(run.status, run.active_decision_position,
                      (SELECT jsonb_agg(requirement.decision_position
                                        ORDER BY requirement.decision_position)
                         FROM storyos.operation_requirements AS requirement
                        WHERE requirement.run_id = run.run_id
                          AND requirement.requirement_role = 'primary'),
                      (SELECT jsonb_path_query_first(requirement.payload, $2::text::jsonpath)
                         FROM storyos.operation_requirements AS requirement
                        WHERE requirement.run_id = run.run_id
                          AND requirement.requirement_role = 'primary'
                          AND requirement.decision_position = run.active_decision_position))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[
                &run_id,
                &r#"$.selected[*] ? (@.source_class == "author_instruction").content"#,
            ],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// A pause opens no position. The steering input that resumes the Run has position 1, so the
/// first request goes out at position 1 and reads the input, and position 0 sends no request
/// (ADR 0062). A queued Run with no pause does the same. Position 0 counts, so the Run sends at
/// most 23 requests.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pause_before_the_first_request_opens_no_position_and_the_input_opens_position_1() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let (paused, queued_only) = (true, false);
    let call = "call_finish";
    let cases = [
        (["fb0", "fb1"], 0xfb90, paused),
        (["fb2", "fb3"], 0xfb92, queued_only),
    ];
    for (prefixes, suffix, pause) in cases {
        let (scope, chapter_id, first) = queued(&store, prefixes[0]).await;
        settle(&store, &admin, &scope, &first, &FakeDestination)
            .await
            .unwrap();
        let conversation = ConversationSelection::New;
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefixes[1],
            tool_loop.clone(),
            MESSAGE,
            conversation,
            None,
        )
        .await;
        if pause {
            let input = PauseAgentRunInput {
                run_id: run_id.clone(),
            };
            let call = issued(&store, &scope, suffix + 1, &PAUSE_AGENT_RUN, input).await;
            store
                .pause_agent_run(&call.envelope, &call.input)
                .await
                .unwrap();
            assert_eq!(
                run_position(&admin, &run_id).await,
                json!(["paused", 0, [0], MESSAGE]),
                "{pause}"
            );
        }
        let input = steering(&store, &scope, suffix, &run_id, "Keep the voice.", None).await;
        store
            .steer_agent_run(&input.envelope, &input.input)
            .await
            .unwrap();
        assert_eq!(
            run_position(&admin, &run_id).await,
            json!(["queued", 0, [0], MESSAGE]),
            "{pause}"
        );
        settle(&store, &admin, &scope, &run_id, &FakeDestination)
            .await
            .unwrap();

        assert_eq!(
            (
                step_requests(&admin, &run_id).await,
                run_position(&admin, &run_id).await,
                steering_facts(&store, &scope, &run_id).await,
            ),
            (
                json!([
                    [1, "tool_request", STEERED, "required", []],
                    [2, "advisory", null, "none", [call]]
                ]),
                json!(["completed", 2, [0, 1, 2], STEERED]),
                json!({"status": format!("{:?}", AgentRunStatus::Completed),
                       "inputs": [["1", "Keep the voice.", true]]}),
            ),
            "{pause}"
        );
    }

    // A Run that never finishes: position 0 sent no request but counts, so the requests go out
    // at positions 1 to 23, and the request at position 23 is the last.
    let (scope, chapter_id, first) = queued(&store, "fb6").await;
    settle(&store, &admin, &scope, &first, &FakeDestination)
        .await
        .unwrap();
    let conversation = ConversationSelection::New;
    let message = "SCRIPT:never_finish";
    let run_id = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "fb7",
        tool_loop,
        message,
        conversation,
        None,
    )
    .await;
    let input = PauseAgentRunInput {
        run_id: run_id.clone(),
    };
    let call = issued(&store, &scope, 0xfb97, &PAUSE_AGENT_RUN, input).await;
    store
        .pause_agent_run(&call.envelope, &call.input)
        .await
        .unwrap();
    let input = steering(&store, &scope, 0xfb96, &run_id, "Keep the voice.", None).await;
    store
        .steer_agent_run(&input.envelope, &input.input)
        .await
        .unwrap();
    settle(&store, &admin, &scope, &run_id, &FakeDestination)
        .await
        .unwrap();
    let steps: Vec<Value> = step_requests(&admin, &run_id)
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|step| json!([step[0], step[3]]))
        .collect();
    let mut expected: Vec<Value> = (1..23)
        .map(|position| json!([position, "required"]))
        .collect();
    expected.push(json!([23, "none"]));

    assert_eq!(steps, expected);
}
