//! The acknowledgement records that the Command Idempotency Fence keeps for an exact retry.

use storyos_application::{
    ChapterId, Project, ProjectAssistanceAcknowledgement, ProjectCommandEnvelope,
    ProjectCommandError,
};
use tokio_postgres::Client;

use super::unavailable;
use crate::command_replay::{CommandReplay, ReplayFault, response_project};
use crate::command_response_assistance::{
    COMMAND_RESPONSE_ASSISTANCE_FORMAT, encode_command_response_assistance,
};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use crate::update_project_assistance::read_assistance_record;

/// The acknowledgement record that the Command Idempotency Fence keeps for an exact retry of one
/// command kind (ADR 0076, ADR 0080).
///
/// The sequence settles the record after every write of a first use, and reads it on an exact
/// retry.
#[derive(Clone, Copy)]
pub(crate) enum CommandResponse {
    /// The Command-response Project.
    Project,
    /// The Command-response Project and the Project assistance record after the writes.
    ProjectAssistance,
    /// No response record: the command decodes its whole acknowledgement from the stored Receipt
    /// and effect rows, so an exact retry never gives `historical_acknowledgement_unavailable`.
    None,
}

const RESPONSE_MISMATCH: &str = "the response record does not match the settlement type";

/// The response record of one settlement. The settlement type of the command converts it.
pub(crate) enum ResponseValue {
    Project(Project),
    ProjectAssistance(ProjectAssistanceAcknowledgement),
    None,
}

/// The refusal of a response record that the admit step does not record.
const ADMIT_RESPONSE: &str = "the admit step records only the Command-response Project";

impl CommandResponse {
    /// Reads the response record after the writes of a first use, and settles the fence.
    pub(super) async fn settle<P: TryFrom<ResponseValue>>(
        self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> Result<P, ProjectCommandError> {
        let value = match self {
            Self::Project => {
                let project = read_response_project(client, envelope).await?;
                settle_fence(
                    client,
                    envelope,
                    command_kind,
                    COMMAND_RESPONSE_PROJECT_FORMAT,
                    &project,
                    /*assistance*/ None,
                )
                .await?;
                ResponseValue::Project(project)
            }
            Self::ProjectAssistance => {
                let project = read_response_project(client, envelope).await?;
                let assistance = read_assistance_record(client, &envelope.project_scope)
                    .await
                    .map_err(unavailable)?;
                settle_fence(
                    client,
                    envelope,
                    command_kind,
                    COMMAND_RESPONSE_ASSISTANCE_FORMAT,
                    &project,
                    Some(encode_command_response_assistance(assistance.as_ref())),
                )
                .await?;
                ResponseValue::ProjectAssistance(ProjectAssistanceAcknowledgement {
                    project,
                    assistance,
                })
            }
            Self::None => {
                settle_fence_without_response(client, envelope, command_kind).await?;
                ResponseValue::None
            }
        };
        P::try_from(value).map_err(|_| unavailable(RESPONSE_MISMATCH))
    }

    /// Reads the response record of an exact retry from the fence.
    pub(super) fn replay<P: TryFrom<ResponseValue>>(
        self,
        replay: &CommandReplay,
    ) -> Result<P, ReplayFault> {
        let value = match self {
            Self::Project => ResponseValue::Project(replay.response_project()?),
            Self::ProjectAssistance => {
                ResponseValue::ProjectAssistance(replay.response_project_assistance()?)
            }
            Self::None => ResponseValue::None,
        };
        P::try_from(value).map_err(|_| ReplayFault::Unavailable(RESPONSE_MISMATCH.into()))
    }

    /// Reads the response record after the work rows of an admit step, and records it on the
    /// fence, which stays `in_progress`. The later settlement keeps the record.
    pub(super) async fn record_admitted<P: TryFrom<ResponseValue>>(
        self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> Result<P, ProjectCommandError> {
        let Self::Project = self else {
            return Err(unavailable(ADMIT_RESPONSE));
        };
        let project = read_response_project(client, envelope).await?;
        let scope = &envelope.project_scope;
        client
            .execute(
                "UPDATE storyos.command_idempotency
                    SET acknowledgement_format = $3, response_project = $4::text::jsonb
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND command_kind = $5 AND idempotency_key = $6::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &COMMAND_RESPONSE_PROJECT_FORMAT,
                    &encode_command_response_project(&project),
                    &command_kind,
                    &envelope.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(unavailable)?;
        P::try_from(ResponseValue::Project(project)).map_err(|_| unavailable(RESPONSE_MISMATCH))
    }

    /// Decodes the response record of an admit step from the fence columns
    /// `acknowledgement_format` and `response_project` of an `in_progress` fence.
    pub(super) fn in_progress<P: TryFrom<ResponseValue>>(
        self,
        format: Option<&str>,
        payload: Option<&str>,
    ) -> Result<P, ReplayFault> {
        let Self::Project = self else {
            return Err(ReplayFault::Unavailable(ADMIT_RESPONSE.into()));
        };
        P::try_from(ResponseValue::Project(response_project(format, payload)?))
            .map_err(|_| ReplayFault::Unavailable(RESPONSE_MISMATCH.into()))
    }
}

impl TryFrom<ResponseValue> for Project {
    type Error = ResponseValue;

    fn try_from(value: ResponseValue) -> Result<Self, ResponseValue> {
        match value {
            ResponseValue::Project(project) => Ok(project),
            value @ (ResponseValue::ProjectAssistance(_) | ResponseValue::None) => Err(value),
        }
    }
}

impl TryFrom<ResponseValue> for ProjectAssistanceAcknowledgement {
    type Error = ResponseValue;

    fn try_from(value: ResponseValue) -> Result<Self, ResponseValue> {
        match value {
            ResponseValue::ProjectAssistance(acknowledgement) => Ok(acknowledgement),
            value @ (ResponseValue::Project(_) | ResponseValue::None) => Err(value),
        }
    }
}

impl TryFrom<ResponseValue> for () {
    type Error = ResponseValue;

    fn try_from(value: ResponseValue) -> Result<Self, ResponseValue> {
        match value {
            ResponseValue::None => Ok(()),
            value @ (ResponseValue::Project(_) | ResponseValue::ProjectAssistance(_)) => Err(value),
        }
    }
}

async fn settle_fence_without_response(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command_kind: &'static str,
) -> Result<(), ProjectCommandError> {
    let scope = &envelope.project_scope;
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = $5 AND idempotency_key = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.challenge_binding.idempotency_key,
                &command_kind,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

async fn read_response_project(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<Project, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let row = client
        .query_one(
            "SELECT title, current_chapter_id::text FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?;
    Ok(Project {
        project_id: scope.project_id.clone(),
        title: row.get(/*idx*/ 0),
        current_chapter_id: row.get::<_, Option<String>>(/*idx*/ 1).map(ChapterId::new),
    })
}

async fn settle_fence(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command_kind: &'static str,
    format: &str,
    project: &Project,
    assistance: Option<String>,
) -> Result<(), ProjectCommandError> {
    let scope = &envelope.project_scope;
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled',
                    result_reference = $3,
                    acknowledgement_format = $5,
                    response_project = $6::text::jsonb,
                    response_assistance = $8::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = $7 AND idempotency_key = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.challenge_binding.idempotency_key,
                &format,
                &encode_command_response_project(project),
                &command_kind,
                &assistance,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}
