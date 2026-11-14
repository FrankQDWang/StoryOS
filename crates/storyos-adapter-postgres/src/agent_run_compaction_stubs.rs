//! The later request of one compaction: its Tool result stubs (ADR 0093 section 5) and its
//! earlier author messages (section 6).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    CurrentPassageAssemblyRecord, keep_earlier_author_messages, later_request_record,
    stub_tool_call_ids,
};

use super::{CompactionState, unavailable, work_error};

/// The Context Assembly of the later request of `staged`. The summary request continued its
/// prior response and sent the outputs of the calls of that response, so they keep their exact
/// form. Each other result went to the model in an earlier decision request, so it is a stub.
/// The recent author messages of the earlier Runs go before the current input (ADR 0093 section
/// 6).
pub(super) async fn later_record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    decision: &CurrentPassageAssemblyRecord,
    staged: &CompactionState,
) -> Result<CurrentPassageAssemblyRecord, CompleteAgentRunError> {
    let prior = crate::agent_run_prior_response::PriorResponse::load(
        client,
        claim,
        crate::agent_run_prior_response::PriorKey::Attempt(&staged.prior_model_attempt_id),
    )
    .await?;
    let results = crate::agent_run_tool_result::RunToolResults::load(client, claim).await?;
    let mut later = later_request_record(
        decision,
        &staged.compaction_id,
        &staged.output_text,
        &results.later_request_candidates(prior.decision_id()),
    );
    keep_earlier_author_messages(&mut later, &super::messages::load(client, claim).await?);
    Ok(later)
}

/// Records on the staged compaction each ToolCall whose result the later request `later` sends
/// as a stub.
pub(super) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    staged: &CompactionState,
    later: &CurrentPassageAssemblyRecord,
) -> Result<(), CompleteAgentRunError> {
    let updated = client
        .execute(
            "UPDATE storyos.active_context_compactions SET stub_tool_call_ids = $4::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND compaction_id = $3::text::uuid AND install_state = 'staged'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &staged.compaction_id,
                &serde_json::json!(stub_tool_call_ids(later)).to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    if updated != 1 {
        return Err(unavailable("The staged compaction is missing"));
    }
    Ok(())
}
