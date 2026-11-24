//! The Update Project Instruction command and the read of the current Revision (ADR 0094 section 2).

use storyos_application::{
    ProjectCommandEnvelope, ProjectInstructionRevision, ProjectInstructionSaved, ProjectReadError,
    ProjectScope, RefusableCommandError, UpdateProjectInstructionError,
    UpdateProjectInstructionInput, UpdateProjectInstructionSettlement,
};
use storyos_contracts::CommandKind;
use storyos_core::{
    TransitionOutcome, UpdateProjectInstruction as CoreUpdateProjectInstruction,
    update_project_instruction,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, AppliedVariant, Classification,
    CommandResponse, CommandSpec, CommandWrite, LockedProject, ProjectCommand, ReplayEffect,
    settle_project_command, unavailable,
};
use crate::{PostgresProjectReader, read_error};

impl PostgresProjectReader {
    /// Settles one save of the Project Instruction as one new immutable Revision.
    pub async fn update_project_instruction(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &UpdateProjectInstructionInput,
    ) -> Result<UpdateProjectInstructionSettlement, UpdateProjectInstructionError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for UpdateProjectInstructionInput {
    const SPEC: CommandSpec<UpdateProjectInstructionError> = CommandSpec {
        kind: CommandKind::UpdateProjectInstruction,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "project_instruction_updated",
        replay_effect: ReplayEffect::NoQuery,
        response: CommandResponse::Project,
        contention_refusal: None,
        receipt_payload: "{}",
    };
    type Error = UpdateProjectInstructionError;
    type Profile = ActivityOnly;
    type Applied = ProjectInstructionSaved;
    type Plan = ();
    type Settlement = UpdateProjectInstructionSettlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, UpdateProjectInstructionError> {
        let current = read_current_revision(client, &envelope.project_scope)
            .await
            .map_err(unavailable)?;
        let (current_revision, current_text) = current
            .map(|revision| (revision.revision, revision.text))
            .unwrap_or_default();
        let outcome = match update_project_instruction(&CoreUpdateProjectInstruction {
            current_revision,
            current_text,
            expected_revision: self.expected_revision,
            text: self.text.clone(),
        }) {
            TransitionOutcome::Applied(applied) => TransitionOutcome::Applied((
                ProjectInstructionSaved {
                    instruction_revision_id: Uuid::now_v7().to_string(),
                    revision: applied.revision,
                },
                (),
            )),
            TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
            TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            TransitionOutcome::Refused(refusal) => {
                return Err(RefusableCommandError::RefusedBeforeAdmission(refusal));
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
        applied: ProjectInstructionSaved,
    ) -> Result<CommandWrite<'_, Self>, storyos_application::ProjectCommandError> {
        client
            .execute(
                "INSERT INTO storyos.project_instruction_revisions
                   (owner_user_id, project_id, instruction_revision_id, revision, text, receipt_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric, $5,
                         $6::text::uuid)",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &applied.instruction_revision_id,
                    &applied.revision.to_string(),
                    &self.text,
                    &envelope.ids.receipt_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(CommandWrite {
            profile: ActivityWrite {
                activity: serde_json::json!({
                    "instruction_revision_id": applied.instruction_revision_id,
                    "revision": applied.revision.to_string(),
                }),
                effect: applied,
            },
            after_authority: None,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProjectInstructionSaved, ReplayFault> {
        Ok(ProjectInstructionSaved {
            instruction_revision_id: replay.activity_uuid("instruction_revision_id")?,
            revision: replay.activity_u64("revision")?,
        })
    }
}

/// The current Revision of the Project Instruction in the scope that `client` has set.
pub(crate) async fn read_current_revision(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
) -> Result<Option<ProjectInstructionRevision>, ProjectReadError> {
    let row = client
        .query_opt(
            "SELECT instruction_revision_id::text, revision::text, text
               FROM storyos.project_instruction_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              ORDER BY project_instruction_revisions.revision DESC
              LIMIT 1",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    row.map(|row| {
        Ok(ProjectInstructionRevision {
            instruction_revision_id: row.get(/*idx*/ 0),
            revision: row
                .get::<_, String>(/*idx*/ 1)
                .parse()
                .map_err(ProjectReadError::unavailable)?,
            text: row.get(/*idx*/ 2),
        })
    })
    .transpose()
}
