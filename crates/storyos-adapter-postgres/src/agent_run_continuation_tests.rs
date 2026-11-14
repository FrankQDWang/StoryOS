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
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};
use crate::agent_run_propose_edit::tests::REVISE;

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
                        '$.selected[*] ? (@.source_class == \"working_target\").content'),
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

async fn proposal_block(admin: &Client, run_id: &str) -> Value {
    let block: String = admin
        .query_one(
            "SELECT operation.manuscript_block_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_operations AS operation USING (owner_user_id, project_id, proposal_id)
              WHERE proposal.source_run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    json!(block)
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
    let (scope, chapter_id, first) =
        queued_run_offering(&store, "cd1", tool_loop.clone(), None).await;
    complete(&store, &admin, &scope, &first).await;
    let edit = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "cd2",
        tool_loop,
        REVISE,
        ConversationSelection::Existing {
            conversation_id: conversation(&admin, &first).await,
        },
        None,
    )
    .await;
    complete(&store, &admin, &scope, &edit).await;
    let first_closing = requests(&admin, &first).await.last().unwrap()[3].clone();
    let steps = requests(&admin, &edit).await;
    let reserved = proposal_block(&admin, &edit).await;
    let targets = steps[0][4].as_array().unwrap().clone();
    let current_targets: Vec<Value> = targets
        .iter()
        .filter(|block| **block != reserved)
        .cloned()
        .collect();

    // The `propose_edit` call of RunStep 0 reserves its block, so the closing request has other
    // targets. It still continues the response of RunStep 0, and it sends the current input.
    assert_eq!(
        (
            steps.clone(),
            targets.contains(&reserved),
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
