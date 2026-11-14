//! The earlier author messages of the later request of one compaction (ADR 0093 section 6).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    CurrentPassageAssemblyRecord, EarlierAuthorMessage, EarlierAuthorMessageSource,
};

use super::{CompactionState, unavailable, work_error};

/// The author message and each steering input that a RunStep read, of each earlier completed Run
/// of the Conversation of the claimed Run, oldest first. The prior response chain of a
/// Conversation holds only completed Runs (ADR 0053), so these messages are the messages that the
/// summary covers.
pub(super) async fn load(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Vec<EarlierAuthorMessage>, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT earlier.run_id::text, message.message_id, message.text
               FROM storyos.agent_runs AS run
               JOIN storyos.agent_runs AS earlier
                 ON (earlier.owner_user_id, earlier.project_id, earlier.conversation_id) =
                    (run.owner_user_id, run.project_id, run.conversation_id)
                AND earlier.run_id < run.run_id AND earlier.status = 'completed'
              CROSS JOIN LATERAL (
                SELECT earlier.run_id::text AS message_id, earlier.author_message AS text,
                       -1::numeric AS position
                UNION ALL
                SELECT input.payload->>'steering_input_id', input.payload->>'author_message',
                       (input.payload->>'input_position')::numeric
                  FROM storyos.project_activity_event_payloads AS input
                 WHERE (input.owner_user_id, input.project_id) =
                       (earlier.owner_user_id, earlier.project_id)
                   AND input.event_kind = 'agent_run_steering_retained'
                   AND input.payload->>'run_id' = earlier.run_id::text
                   AND (input.payload->>'input_position')::numeric <=
                       earlier.active_decision_position
              ) AS message
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid
              ORDER BY earlier.run_id, message.position",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(rows
        .iter()
        .map(|row| EarlierAuthorMessage {
            source: EarlierAuthorMessageSource {
                run_id: row.get(/*idx*/ 0),
                message_id: row.get(/*idx*/ 1),
            },
            text: row.get(/*idx*/ 2),
        })
        .collect())
}

/// Records on the staged compaction each earlier author message that the later request `later`
/// sends.
pub(super) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    staged: &CompactionState,
    later: &CurrentPassageAssemblyRecord,
) -> Result<(), CompleteAgentRunError> {
    let kept: Vec<&str> = later
        .operation_requirement
        .earlier_author_messages
        .iter()
        .flatten()
        .map(|source| source.message_id.as_str())
        .collect();
    let updated = client
        .execute(
            "UPDATE storyos.active_context_compactions
                SET kept_author_message_ids = $4::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND compaction_id = $3::text::uuid AND install_state = 'staged'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &staged.compaction_id,
                &serde_json::json!(kept).to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    if updated != 1 {
        return Err(unavailable("The staged compaction is missing"));
    }
    Ok(())
}
