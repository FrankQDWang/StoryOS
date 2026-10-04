//! The `ActivityOnly` settlement profile: one Project Activity record and no Authoritative
//! Commit, Author Action, Snapshot, or Manuscript Tree Revision change (ADR 0043).

use storyos_application::{
    ActivityApplied, ProjectCommandEnvelope, ProjectCommandError, ProjectScope,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::records::insert_applied_activity;
use super::{LockedProject, SettlementProfile, unavailable};
use crate::command_replay::{CommandReplay, ReplayFault};

/// The applied writes that one `ActivityOnly` command returns.
pub(crate) struct ActivityWrite<E> {
    pub(crate) effect: E,
    /// The command fields of the Activity payload; the profile adds `kind`.
    pub(crate) activity: serde_json::Value,
}

pub(crate) struct ActivitySequences {
    project_activity_position: u64,
    project_activity_event_id: String,
}

pub(crate) struct ActivityOnly;

impl SettlementProfile for ActivityOnly {
    type Sequences = ActivitySequences;
    type Write<E: Send> = ActivityWrite<E>;
    type Applied<E: Send> = ActivityApplied<E>;

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<ActivitySequences, ProjectCommandError> {
        let position = client
            .query_one(
                "INSERT INTO storyos.scope_counters AS counters
                   (owner_user_id, project_id, project_activity_position)
                 VALUES ($1::text::uuid, $2::text::uuid, 1)
                 ON CONFLICT (owner_user_id, project_id)
                 DO UPDATE SET
                   project_activity_position = counters.project_activity_position + 1
                 RETURNING counters.project_activity_position::text",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(unavailable)?
            .get::<_, String>(0);
        Ok(ActivitySequences {
            project_activity_position: position.parse().map_err(unavailable)?,
            project_activity_event_id: Uuid::now_v7().to_string(),
        })
    }

    fn commit_ids(_sequences: &ActivitySequences) -> Vec<String> {
        Vec::new()
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        activity_kind: &'static str,
        sequences: ActivitySequences,
        write: ActivityWrite<E>,
    ) -> Result<ActivityApplied<E>, ProjectCommandError> {
        insert_applied_activity(
            client,
            envelope,
            activity_kind,
            sequences.project_activity_position,
            &sequences.project_activity_event_id,
            write.activity,
            &[],
        )
        .await?;
        Ok(ActivityApplied {
            effect: write.effect,
            project_activity_position: sequences.project_activity_position,
            project_activity_event_id: sequences.project_activity_event_id,
        })
    }

    fn replay<E: Send>(
        effect: E,
        replay: &CommandReplay,
    ) -> Result<ActivityApplied<E>, ReplayFault> {
        Ok(ActivityApplied {
            effect,
            project_activity_position: replay.project_activity_position,
            project_activity_event_id: replay.project_activity_event_id.clone(),
        })
    }
}
