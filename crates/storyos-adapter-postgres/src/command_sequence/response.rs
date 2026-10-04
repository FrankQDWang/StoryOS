//! The acknowledgement records that the Command Idempotency Fence keeps for an exact retry.

use std::future::Future;

use storyos_application::{
    ChapterId, Project, ProjectAssistanceAcknowledgement, ProjectCommandEnvelope,
    ProjectCommandError,
};
use tokio_postgres::Client;

use super::unavailable;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_response_assistance::{
    COMMAND_RESPONSE_ASSISTANCE_FORMAT, encode_command_response_assistance,
};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use crate::update_project_assistance::read_assistance_record;

/// The acknowledgement record of one command kind (ADR 0043).
///
/// The sequence calls `settle` after every write of a first use; it reads the response and
/// settles the Command Idempotency Fence. On an exact retry it calls `replay` instead.
pub(crate) trait ResponseRecord {
    type Response: Send;

    fn settle(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> impl Future<Output = Result<Self::Response, ProjectCommandError>> + Send;

    fn replay(replay: &CommandReplay) -> Result<Self::Response, ReplayFault>;
}

/// The Command-response Project.
pub(crate) struct ProjectResponse;

/// No response record: the command decodes its whole acknowledgement from the stored Receipt
/// and effect rows, so an exact retry never gives `historical_acknowledgement_unavailable`.
pub(crate) struct NoResponse;

/// The Command-response Project and the Project assistance record after the writes.
pub(crate) struct ProjectAssistanceResponse;

impl ResponseRecord for ProjectResponse {
    type Response = Project;

    async fn settle(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> Result<Project, ProjectCommandError> {
        let project = read_response_project(client, envelope).await?;
        settle_fence(
            client,
            envelope,
            command_kind,
            COMMAND_RESPONSE_PROJECT_FORMAT,
            &project,
            None,
        )
        .await?;
        Ok(project)
    }

    fn replay(replay: &CommandReplay) -> Result<Project, ReplayFault> {
        replay.response_project()
    }
}

impl ResponseRecord for NoResponse {
    type Response = ();

    async fn settle(
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

    fn replay(_replay: &CommandReplay) -> Result<(), ReplayFault> {
        Ok(())
    }
}

impl ResponseRecord for ProjectAssistanceResponse {
    type Response = ProjectAssistanceAcknowledgement;

    async fn settle(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> Result<ProjectAssistanceAcknowledgement, ProjectCommandError> {
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
        Ok(ProjectAssistanceAcknowledgement {
            project,
            assistance,
        })
    }

    fn replay(replay: &CommandReplay) -> Result<ProjectAssistanceAcknowledgement, ReplayFault> {
        replay.response_project_assistance()
    }
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
        title: row.get(0),
        current_chapter_id: row.get::<_, Option<String>>(1).map(ChapterId::new),
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
