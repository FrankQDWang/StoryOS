//! The Provider conversation of a later RunStep whose current input changed (ADR 0085), with the
//! fake model of the tool loop profile.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ConversationSelection, NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::DeploymentDestination;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_referenced_run, admit_run_in, claim_run, stores,
};
use crate::agent_run_propose_edit::revise_tests::{CandidateChange, Fixture};
use crate::agent_run_propose_edit::tests::REVISE;
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle, steering};

async fn complete(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run(
        store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
}

/// For each decision Model Attempt of the Run in RunStep order: its position, its input mapping,
/// the prior response that its request named, its own response, the target blocks and the
/// working text that its request sent, and the working text of its Context Assembly record.
async fn requests(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(
                      attempt.decision_position,
                      attempt.payload #>> '{continuation,input_mapping}',
                      wire ->> 'previous_response_reference',
                      attempt.payload ->> 'response_reference',
                      jsonb_path_query_array(wire, '$.declared_targets[*].block_id'),
                      jsonb_path_query_array(wire,
                        '$.context[*] ? (@.source_class == \"working_target\").content'),
                      (SELECT jsonb_path_query_array(manifest.payload,
                                '$.selected[*] ? (@.source_class == \"working_target\").content')
                         FROM storyos.context_assembly_manifests AS manifest
                        WHERE manifest.run_id = attempt.run_id
                          AND manifest.decision_position = attempt.decision_position
                          AND manifest.manifest_role = 'decision'))::text
               FROM storyos.model_attempts AS attempt,
                    LATERAL (SELECT (attempt.payload #>> '{wire,serialized_payload}')::jsonb)
                      AS w(wire)
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'decision'
              ORDER BY attempt.decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

async fn conversation(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// Each block that a Proposal of the Run reserves.
async fn reserved_blocks(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT operation.manuscript_block_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_operations AS operation USING (owner_user_id, project_id, proposal_id)
              WHERE proposal.source_run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| json!(row.get::<_, String>(/*idx*/ 0)))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_request_after_an_opened_proposal_continues_the_prior_response_with_the_current_input()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0xcd0, tool_loop.clone()).await;
    let (scope, first) = (&fixture.scope, &fixture.source_run);
    let edit = admit_run_in(
        &store,
        scope,
        &fixture.chapter_id,
        "cd8",
        tool_loop,
        REVISE,
        ConversationSelection::Existing {
            conversation_id: conversation(&admin, first).await,
        },
        None,
    )
    .await;
    complete(&store, &admin, scope, &edit).await;
    let first_closing = requests(&admin, first).await.last().unwrap()[3].clone();
    let steps = requests(&admin, &edit).await;
    let reserved = reserved_blocks(&admin, &edit).await;
    let targets = steps[0][4].as_array().unwrap().clone();
    let current_targets: Vec<Value> = targets
        .iter()
        .filter(|block| !reserved.contains(block))
        .cloned()
        .collect();

    // The `propose_edit` call of RunStep 0 reserves its block, so the closing request has other
    // targets. It still continues the response of RunStep 0, and it sends the current input.
    assert_eq!(
        (
            steps.clone(),
            reserved.iter().all(|block| targets.contains(block)),
            current_targets.len() < targets.len(),
        ),
        (
            vec![
                json!([
                    0,
                    "incremental",
                    first_closing,
                    steps[0][3],
                    targets,
                    steps[0][6],
                    steps[0][6],
                ]),
                json!([
                    1,
                    "incremental",
                    steps[0][3],
                    steps[1][3],
                    current_targets,
                    steps[1][6],
                    steps[1][6],
                ]),
            ],
            true,
            true,
        )
    );
    assert_ne!(steps[1][6], json!([]));
}

/// For the RunStep at `position`: its input mapping, whether its request named the response of
/// the RunStep before it, whether the request says that its input replaces the earlier input, and
/// whether the request sent `text`.
async fn continued(admin: &Client, run_id: &str, position: u64, text: &str) -> Value {
    let steps = requests(admin, run_id).await;
    let row = admin
        .query_one(
            "SELECT payload #>> '{wire,serialized_payload}'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                AND decision_position = $2::text::numeric",
            &[&run_id, &position.to_string()],
        )
        .await
        .unwrap();
    let wire: Value = serde_json::from_str(row.get(/*idx*/ 0)).unwrap();
    let index = usize::try_from(position).unwrap();
    json!([
        steps[index][1],
        steps[index][2] == steps[index - 1][3],
        wire["replaces_earlier_input"],
        wire.to_string().contains(text),
    ])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_step_after_a_steering_input_an_author_edit_or_a_head_move_continues_the_prior_response()
 {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let message = "Help with this passage.";
    let edited = "Still, ";
    // A steering input while the first request is out.
    let (scope, _, steered) = queued(&store, "cca").await;
    let input = steering(&store, &scope, 0xccb0, &steered, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &steered, &author)
        .await
        .unwrap();
    // An Author Edit of the referenced candidate while the first request is out.
    let candidate = Fixture::new(&store, &admin, 0xce0, tool_loop.clone()).await;
    let reference = candidate.candidates(&admin).await.remove(0);
    let candidate_run = admit_referenced_run(
        &store,
        &candidate.scope,
        &candidate.chapter_id,
        "ce8",
        tool_loop.clone(),
        message,
        vec![reference.clone()],
    )
    .await;
    let edit = candidate
        .candidate_edit(
            &store,
            &admin,
            &reference,
            2,
            CandidateChange::Prefix(edited),
        )
        .await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, &candidate.scope, &candidate_run, &author)
        .await
        .unwrap();
    // An Author Edit of a referenced Chapter Block, which moves the head, while the first request
    // is out.
    let chapter = Fixture::new(&store, &admin, 0xcc0, tool_loop.clone()).await;
    let chapter_run = admit_referenced_run(
        &store,
        &chapter.scope,
        &chapter.chapter_id,
        "cc8",
        tool_loop,
        message,
        vec![chapter.chapter_text(1, 2)],
    )
    .await;
    let edit = chapter
        .chapter_edit(
            &store,
            2,
            vec![storyos_core::AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: chapter.block(1).to_owned(),
                from: 0,
                to: 0,
                text: edited.to_owned(),
            }],
        )
        .await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, &chapter.scope, &chapter_run, &author)
        .await
        .unwrap();

    // The next RunStep of each Run continues the response of the first RunStep, and it sends the
    // changed input.
    assert_eq!(
        [
            continued(&admin, &steered, 1, "Keep the voice.").await,
            continued(&admin, &candidate_run, 1, edited).await,
            continued(&admin, &chapter_run, 1, "Still, Wind rose.").await,
        ],
        [
            json!(["incremental", true, true, true]),
            json!(["incremental", true, true, true]),
            json!(["incremental", true, true, true]),
        ]
    );
}
