use std::convert::Infallible;

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, UpdateProjectInput, UpdateProjectSettlement,
};
use storyos_core::{
    ProjectLifecycle, TransitionOutcome, UpdateProject as CoreUpdateProject, UpdateProjectApplied,
    UpdateProjectConflict, UpdateProjectNoEffect, UpdateProjectRefusal,
    update_project as classify_update_project,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, AppliedResult, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, ProjectCommand,
    ProjectResponse, settle_project_command, unavailable,
};

impl PostgresProjectReader {
    /// Settles one Update Project (rename) as a Project setting command.
    pub async fn update_project(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &UpdateProjectInput,
    ) -> Result<UpdateProjectSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for UpdateProjectInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "updateProject",
        applied_result: AppliedResult::AuthoritativeApplied,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        activity_kind: "project_updated",
    };
    type Profile = ActivityOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = UpdateProjectApplied;
    type Plan = ();
    type Effect = UpdateProjectApplied;
    type NoEffect = UpdateProjectNoEffect;
    type Conflict = UpdateProjectConflict;
    type Refusal = Infallible;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        if project.lifecycle == ProjectLifecycle::Archived {
            return Err(ProjectCommandError::BindingConflict);
        }
        let row = client
            .query_one(
                "SELECT title, revision::text FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let classified = classify_update_project(&CoreUpdateProject {
            expected_revision: self.expected_revision,
            current_revision: row
                .get::<_, String>(/*idx*/ 1)
                .parse()
                .map_err(unavailable)?,
            title: self.title.clone(),
            current_title: row.get(/*idx*/ 0),
        });
        let outcome = match classified {
            TransitionOutcome::Applied(applied) => TransitionOutcome::Applied((applied, ())),
            TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
            TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            TransitionOutcome::Refused(UpdateProjectRefusal::InvalidTitle) => {
                return Err(ProjectCommandError::BindingConflict);
            }
        };
        Ok(Classification::project_command(outcome))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        _plan: (),
        applied: UpdateProjectApplied,
    ) -> Result<ActivityWrite<UpdateProjectApplied>, ProjectCommandError> {
        let updated = client
            .execute(
                "UPDATE storyos.projects
                    SET title = $3, revision = $4::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND revision = $5::text::bigint",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &applied.title,
                    &applied.revision.to_string(),
                    &self.expected_revision.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated != 1 {
            return Err(unavailable("Project revision changed under FOR UPDATE"));
        }
        Ok(ActivityWrite {
            activity: serde_json::json!({
                "title": applied.title,
                "revision": applied.revision.to_string(),
            }),
            effect: applied,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<UpdateProjectApplied, ReplayFault> {
        Ok(UpdateProjectApplied {
            title: replay.activity_text("title")?,
            revision: replay.activity_u64("revision")?,
        })
    }
}
