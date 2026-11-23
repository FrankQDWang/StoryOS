//! The earlier turns of the Project Conversation of a Run of the author, from the Host records
//! (ADR 0102).

use storyos_application::{CreateAgentRunError, ProjectScope};
use storyos_core::{EarlierMessageKind, EarlierTurn, EarlierTurnMessage, EarlierTurnSource};

use super::agent_run_database_error;

/// The messages of each earlier completed Run of the author in the Conversation of `run_id`,
/// oldest first: the author message, each steering input that a RunStep read, and the final
/// answer text. A Run that is refused, cancelled, or in progress gives no turn. After a compaction
/// of the Run is installed, the summary is in the place of the earlier turns, so there is none.
pub(crate) async fn load(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<EarlierTurn>, CreateAgentRunError> {
    let rows = client
        .query(
            "SELECT earlier.run_id::text, message.message_id, message.author, message.text
               FROM storyos.agent_runs AS run
               JOIN storyos.agent_runs AS earlier
                 ON (earlier.owner_user_id, earlier.project_id, earlier.conversation_id) =
                    (run.owner_user_id, run.project_id, run.conversation_id)
                AND earlier.run_id < run.run_id AND earlier.run_kind = 'author'
                AND earlier.status = 'completed'
              CROSS JOIN LATERAL (
                SELECT earlier.run_id::text AS message_id, true AS author,
                       earlier.author_message AS text, -1::numeric AS position
                UNION ALL
                SELECT input.payload->>'steering_input_id', true, input.payload->>'author_message',
                       (input.payload->>'input_position')::numeric
                  FROM storyos.project_activity_event_payloads AS input
                 WHERE (input.owner_user_id, input.project_id) =
                       (earlier.owner_user_id, earlier.project_id)
                   AND input.event_kind = 'agent_run_steering_retained'
                   AND input.payload->>'run_id' = earlier.run_id::text
                   AND (input.payload->>'input_position')::numeric <=
                       earlier.active_decision_position
                UNION ALL
                (SELECT attempt.model_attempt_id::text, false,
                        attempt.payload #>> '{decision,text}', 1e30
                   FROM storyos.model_attempts AS attempt
                  WHERE (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                        (earlier.owner_user_id, earlier.project_id, earlier.run_id)
                    AND attempt.attempt_role = 'decision'
                    AND attempt.decision_position = earlier.active_decision_position
                    AND attempt.payload #>> '{decision,kind}' = 'advisory'
                    AND attempt.payload #> '{decision,selected}' = 'true'::jsonb
                    AND COALESCE(attempt.payload #>> '{decision,text}', '') <> ''
                  LIMIT 1)
              ) AS message
              WHERE (run.owner_user_id, run.project_id, run.run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.active_context_compactions AS compaction
                   WHERE (compaction.owner_user_id, compaction.project_id, compaction.run_id) =
                         (run.owner_user_id, run.project_id, run.run_id)
                     AND compaction.install_state = 'installed')
              ORDER BY earlier.run_id, message.position, message.message_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let mut turns: Vec<EarlierTurn> = Vec::new();
    for row in rows {
        let message = EarlierTurnMessage {
            source: EarlierTurnSource {
                run_id: row.get(/*idx*/ 0),
                message_id: row.get(/*idx*/ 1),
                message_kind: match row.get(/*idx*/ 2) {
                    true => EarlierMessageKind::Author,
                    false => EarlierMessageKind::Answer,
                },
            },
            text: row.get(/*idx*/ 3),
        };
        match turns.last_mut() {
            Some(turn) if turn.messages[0].source.run_id == message.source.run_id => {
                turn.messages.push(message);
            }
            _ => turns.push(EarlierTurn {
                messages: vec![message],
            }),
        }
    }
    Ok(turns)
}
