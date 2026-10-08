use std::convert::Infallible;

use storyos_application::{
    ArchiveProjectInput, ArchiveProjectSettlement, ProjectCommandEnvelope, ProjectCommandError,
};
use storyos_core::{
    ArchiveProject as CoreArchiveProject, ArchiveProjectApplied, ArchiveProjectConflict,
    ArchiveProjectNoEffect, archive_project as classify_archive_project,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, AppliedVariant, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, ProjectCommand,
    ProjectResponse, RateLimitedChallenge, ReplayEffect, settle_project_command, unavailable,
};

impl PostgresProjectReader {
    /// Settles one Archive Project as a Project setting command.
    pub async fn archive_project(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ArchiveProjectInput,
    ) -> Result<ArchiveProjectSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for ArchiveProjectInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "archiveProject",
        applied: &[AppliedVariant::NoAuthorAction],
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "project_archival_changed",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Error = ProjectCommandError;
    type Profile = ActivityOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = ArchiveProjectApplied;
    type Plan = ();
    type Effect = ArchiveProjectApplied;
    type NoEffect = ArchiveProjectNoEffect;
    type Conflict = ArchiveProjectConflict;
    type Refusal = Infallible;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let current_revision = client
            .query_one(
                "SELECT revision::text FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?
            .get::<_, String>(/*idx*/ 0)
            .parse()
            .map_err(unavailable)?;
        Ok(Classification::project_command(
            classify_archive_project(&CoreArchiveProject {
                expected_revision: self.expected_revision,
                current_revision,
                current_lifecycle: project.lifecycle,
            })
            .map_applied(|applied| (applied, ())),
        ))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        _plan: (),
        applied: ArchiveProjectApplied,
    ) -> Result<ActivityWrite<ArchiveProjectApplied>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let revision = applied.revision.to_string();
        let updated = client
            .execute(
                "UPDATE storyos.projects
                    SET lifecycle_state = 'archived', revision = $3::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &revision,
                    &self.expected_revision.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated != 1 {
            return Err(unavailable("Project revision changed under FOR UPDATE"));
        }
        client
            .execute(
                "INSERT INTO storyos.project_archival_decisions
                   (owner_user_id, project_id, archival_decision_id, receipt_id,
                    prior_lifecycle_state, resulting_lifecycle_state, project_revision)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         'active', 'archived', $5::text::bigint)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &envelope.ids.receipt_id,
                    &revision,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(ActivityWrite {
            activity: serde_json::json!({ "lifecycle": "archived", "revision": revision }),
            effect: applied,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ArchiveProjectApplied, ReplayFault> {
        Ok(ArchiveProjectApplied {
            revision: replay.activity_u64("revision")?,
        })
    }
}
