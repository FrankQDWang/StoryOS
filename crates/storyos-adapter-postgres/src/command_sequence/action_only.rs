//! The `ActionOnly` settlement profile: one Forward Author Action and no Project Activity
//! record, Authoritative Commit, or Snapshot (ADR 0043).

use storyos_application::{
    ActionApplied, ProjectCommandEnvelope, ProjectCommandError, ProjectScope,
};
use tokio_postgres::Client;

use super::{CommandSpec, LockedProject, SettlementProfile, unavailable};
use crate::command_replay::{CommandReplay, ReplayFault};

/// The Author Action Sequence position that one Forward Author Action uses.
pub(crate) struct ActionSequence(pub(crate) u64);

pub(crate) struct ActionOnly;

impl SettlementProfile for ActionOnly {
    type Sequences = ActionSequence;
    type Write<E: Send> = E;
    type Applied<E: Send> = ActionApplied<E>;

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<ActionSequence, ProjectCommandError> {
        let sequence = client
            .query_one(
                "INSERT INTO storyos.scope_counters AS counters
                   (owner_user_id, project_id, author_action_sequence)
                 VALUES ($1::text::uuid, $2::text::uuid, 1)
                 ON CONFLICT (owner_user_id, project_id)
                 DO UPDATE SET author_action_sequence = counters.author_action_sequence + 1
                 RETURNING counters.author_action_sequence::text",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(unavailable)?
            .get::<_, String>(/*idx*/ 0);
        Ok(ActionSequence(sequence.parse().map_err(unavailable)?))
    }

    fn commit_ids(_sequences: &ActionSequence) -> Vec<String> {
        Vec::new()
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        spec: &CommandSpec,
        ActionSequence(sequence): ActionSequence,
        effect: E,
    ) -> Result<ActionApplied<E>, ProjectCommandError> {
        client
            .execute(
                "INSERT INTO storyos.author_action_entries
                   (owner_user_id, project_id, author_action_sequence, disposition,
                    receipt_id, receipt_result_kind)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                         $4::text::uuid, $5)",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &sequence.to_string(),
                    &envelope.ids.receipt_id,
                    &spec.applied_result.code(),
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(ActionApplied {
            effect,
            author_action_sequence: sequence,
        })
    }

    fn replay<E: Send>(effect: E, replay: &CommandReplay) -> Result<ActionApplied<E>, ReplayFault> {
        let sequence = replay.author_action_sequence.as_deref().ok_or_else(|| {
            ReplayFault::Unavailable("the applied Receipt has no Forward Author Action".into())
        })?;
        Ok(ActionApplied {
            effect,
            author_action_sequence: sequence
                .parse()
                .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?,
        })
    }
}
