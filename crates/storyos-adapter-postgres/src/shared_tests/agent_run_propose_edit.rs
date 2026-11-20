//! The shared test support from `agent_run_propose_edit_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{RuntimeClock, claim_run};
use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{NoContractFaults, ProjectScope, complete_agent_run};
use storyos_core::PROPOSAL_WAITS_RESULT;
use tokio_postgres::Client;

pub(crate) const REVISE: &str = "Revise these passages in the narrator voice.";

pub(crate) async fn complete(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run: &str,
) {
    let claim = claim_run(admin, scope.clone(), run.to_owned()).await;
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

/// The Proposal records of the Run with no identity: the Proposal, its Revisions, its
/// Operations, and its Validation Receipts.
pub(crate) async fn proposal_records(admin: &Client, run_id: &str) -> Value {
    json_of(admin, "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                      'proposal', jsonb_build_array(proposal.kind, proposal.bundle_policy),
                      'revisions', (SELECT jsonb_agg(jsonb_build_array(revision.generation,
                                      revision.validation, revision.closure, revision.candidate_text)
                                      ORDER BY revision.revision_id)
                                      FROM storyos.proposal_revisions AS revision
                                     WHERE revision.proposal_id = proposal.proposal_id),
                      'operations', (SELECT jsonb_agg(jsonb_build_array(operation.resolution,
                                       operation.reservation_state, operation.candidate_text,
                                       cardinality(operation.predecessor_operation_ids))
                                       ORDER BY operation.operation_id)
                                       FROM storyos.proposal_operations AS operation
                                      WHERE operation.proposal_id = proposal.proposal_id),
                      'receipts', (SELECT jsonb_agg(jsonb_build_array(receipt.result,
                                     receipt.candidate_text, receipt.reservation_state))
                                     FROM storyos.validation_receipts AS receipt
                                    WHERE receipt.proposal_id = proposal.proposal_id))
                      ORDER BY proposal.proposal_id), '[]')::text
               FROM storyos.proposals AS proposal
              WHERE proposal.source_run_id = $1::text::uuid", &[&run_id]).await
}

/// Each `propose_edit` call of the Run in RunStep and item order: its RunStep, state, refusal,
/// and result.
pub(crate) async fn propose_edit_calls(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(decision_position, state, refusal, result_text)::text
               FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'propose_edit'
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

pub(crate) fn opened(position: u64) -> Value {
    json!([position, "settled", null, PROPOSAL_WAITS_RESULT])
}
