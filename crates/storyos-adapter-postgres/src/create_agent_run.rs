use std::convert::Infallible;

use storyos_application::{
    AgentRunReadStore, AgentRunRecord, ConversationSelection, CreateAgentRunApplied,
    CreateAgentRunCommandError, CreateAgentRunError, CreateAgentRunInput, CreateAgentRunSettlement,
    ProjectAssistanceRecord, ProjectCommandChallengeError, ProjectCommandEnvelope,
    ProjectCommandError, ProjectScope, RefusableCommandError,
};
use storyos_core::{
    AssistanceAdmission, AssistanceAvailability, ChapterAdmission, ConversationAdmission,
    CreateAgentRun as CoreCreateAgentRun, CreateAgentRunRefusal, ProjectPresence, create_agent_run,
};
use tokio_postgres::Client;

use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedVariant, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, ProjectActionClass,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect, ZeroReceipt,
    settle_project_command, unavailable,
};
use crate::update_project_assistance::read_assistance_record;

use super::*;

#[path = "create_agent_run_context.rs"]
pub(crate) mod context;
#[path = "create_agent_run_read.rs"]
mod read;
#[path = "create_agent_run_write.rs"]
mod write;

impl PostgresProjectReader {
    /// Settles one createAgentRun. A refusal of its facts and a transaction race are refusals
    /// before Admission.
    pub async fn create_agent_run(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &CreateAgentRunInput,
    ) -> Result<CreateAgentRunSettlement, CreateAgentRunCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

impl AgentRunReadStore for PostgresProjectReader {
    async fn read_agent_run(
        &self,
        scope: &ProjectScope,
        run_id: &str,
        selection: &storyos_application::AgentRunReadSelection,
    ) -> Result<Option<AgentRunRecord>, CreateAgentRunError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(agent_run_challenge_error)?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(agent_run_database_error)?;
        let result = async {
            set_challenge_scope_on_client(&client, scope)
                .await
                .map_err(agent_run_challenge_error)?;
            read::load_agent_run(&client, scope, run_id, selection).await
        }
        .await;
        match &result {
            Ok(_) => client
                .batch_execute("COMMIT")
                .await
                .map_err(agent_run_database_error)?,
            Err(_) => {
                let _rollback = client.batch_execute("ROLLBACK").await;
            }
        }
        result
    }
}

impl ProjectCommand for CreateAgentRunInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "createAgentRun",
        applied: &[AppliedVariant::NoAuthorAction],
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "agent_run_created",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object('run_id', run_id::text)::text
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND receipt_id = $3::text::uuid",
        ),
    };
    type Error = CreateAgentRunCommandError;
    type Profile = ActivityOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = ();
    type Plan = AssistancePlan;
    type Effect = CreateAgentRunApplied;
    type NoEffect = Infallible;
    type Conflict = Infallible;
    type Refusal = Infallible;

    fn contention_refusal() -> Option<CreateAgentRunCommandError> {
        Some(RefusableCommandError::RefusedBeforeAdmission(
            CreateAgentRunRefusal::ConversationBusy,
        ))
    }

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, CreateAgentRunCommandError> {
        let scope = &envelope.project_scope;
        let assistance = read_assistance_record(client, scope)
            .await
            .map_err(unavailable)?;
        let conversation = match &self.conversation {
            ConversationSelection::New => ConversationAdmission::New,
            ConversationSelection::Existing { conversation_id } => {
                conversation_admission(client, scope, conversation_id).await?
            }
        };
        let outcome = create_agent_run(&CoreCreateAgentRun {
            presence: ProjectPresence::Present,
            lifecycle: project.lifecycle,
            assistance: match (&assistance, &self.destination) {
                (Some(record), _) if record.availability == AssistanceAvailability::Unavailable => {
                    AssistanceAdmission::Unavailable
                }
                (_, None) => AssistanceAdmission::Unavailable,
                (_, Some(_)) => AssistanceAdmission::Available,
            },
            conversation,
            chapter: if project.current_chapter_id.as_deref() == Some(self.chapter_id.as_str()) {
                ChapterAdmission::Current
            } else {
                ChapterAdmission::Invalid
            },
        })
        .map_err(RefusableCommandError::RefusedBeforeAdmission)?;
        if let Some(target) = &self.candidate_target
            && crate::candidate_revision_target::load(client, scope, &self.chapter_id, target)
                .await
                .map_err(sequence_error)?
                .is_none()
        {
            return Err(ProjectCommandError::BindingConflict.into());
        }
        let destination =
            self.destination
                .clone()
                .ok_or(RefusableCommandError::RefusedBeforeAdmission(
                    CreateAgentRunRefusal::AssistanceUnavailable,
                ))?;
        let assistance = match assistance {
            Some(record) if record.destination == destination.kind() => {
                AssistancePlan::Current(record)
            }
            Some(record) => AssistancePlan::Bind {
                revision: record.revision + 1,
                destination,
            },
            None => AssistancePlan::Bind {
                revision: 1,
                destination,
            },
        };
        hold_conversation_if_requested(&envelope.challenge_binding.idempotency_key).await;
        Ok(Classification {
            outcome: outcome.map_applied(|()| ((), assistance)),
            admission: Admission::Project(ProjectActionClass::AgentRunStart),
            heads: ReceiptHeads::default(),
            zero_receipt: ZeroReceipt::Reason,
        })
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        assistance: AssistancePlan,
        _applied: (),
    ) -> Result<ActivityWrite<CreateAgentRunApplied>, ProjectCommandError> {
        let assistance = match assistance {
            AssistancePlan::Current(record) => record,
            AssistancePlan::Bind {
                revision,
                destination,
            } => bind_deployment_destination(client, envelope, &destination, revision).await?,
        };
        let applied = write::persist_conversation_and_run(
            client,
            envelope,
            self,
            &assistance.grant_id,
            &assistance.project_model_use_binding_revision,
        )
        .await
        .map_err(sequence_error)?;
        context::persist_current_passage_assembly(
            client,
            &context::PassageContextInput {
                project_scope: &envelope.project_scope,
                run_id: &self.run_id,
                chapter_id: &self.chapter_id,
                author_message: &self.author_message,
                receipt_id: &envelope.ids.receipt_id,
                decision_position: "0",
                passage_targets: self.passage_targets.as_deref(),
                candidate_target: self.candidate_target.as_ref(),
            },
            &assistance.processing_destination_identity,
        )
        .await
        .map_err(sequence_error)?;
        Ok(ActivityWrite {
            activity: serde_json::json!({
                "project_agent_id": applied.project_agent_id,
                "conversation_id": applied.conversation_id,
                "memory_settings_revision": applied.memory_settings_revision,
                "run_id": applied.run_id,
            }),
            effect: applied,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<CreateAgentRunApplied, ReplayFault> {
        let applied = CreateAgentRunApplied {
            project_agent_id: replay.activity_uuid("project_agent_id")?,
            conversation_id: replay.activity_uuid("conversation_id")?,
            memory_settings_revision: replay.activity_uuid("memory_settings_revision")?,
            run_id: replay.activity_uuid("run_id")?,
        };
        if replay.effect_text("run_id")?.as_deref() != Some(applied.run_id.as_str()) {
            return Err(ReplayFault::Unavailable(
                "the AgentRun of the applied Receipt is missing".into(),
            ));
        }
        Ok(applied)
    }
}

/// The assistance record whose grant and Model Use Binding revision the Run captures.
pub(crate) enum AssistancePlan {
    Current(ProjectAssistanceRecord),
    /// The Project has no binding, or its binding has another destination than the deployment.
    Bind {
        revision: u64,
        destination: storyos_core::DeploymentDestination,
    },
}

/// Makes the deployment destination current with an available policy revision (ADR 0048).
async fn bind_deployment_destination(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    destination: &storyos_core::DeploymentDestination,
    revision: u64,
) -> Result<ProjectAssistanceRecord, ProjectCommandError> {
    let decision = crate::project_destination_binding::insert_destination_binding(
        client,
        envelope,
        destination,
    )
    .await
    .map_err(unavailable)?;
    crate::update_project_assistance::insert_policy_revision(
        client,
        envelope,
        AssistanceAvailability::Available,
        revision,
        Some(&decision),
    )
    .await?;
    read_assistance_record(client, &envelope.project_scope)
        .await
        .map_err(unavailable)?
        .ok_or_else(|| unavailable(std::io::Error::other("the new binding is not current")))
}

async fn conversation_admission(
    client: &Client,
    scope: &ProjectScope,
    conversation_id: &str,
) -> Result<ConversationAdmission, ProjectCommandError> {
    let found = client
        .query_opt(
            "SELECT 1
               FROM storyos.project_conversations
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND conversation_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .is_some();
    if !found {
        return Ok(ConversationAdmission::ExistingMissing);
    }
    let busy = client
        .query_opt(
            "SELECT 1
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND conversation_id = $3::text::uuid
                AND status IN ('queued', 'claimed', 'waiting', 'paused')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .is_some();
    Ok(if busy {
        ConversationAdmission::ExistingBusy
    } else {
        ConversationAdmission::ExistingIdle
    })
}

async fn hold_conversation_if_requested(idempotency_key: &str) {
    let Ok(expected) = std::env::var("STORYOS_TEST_CONVERSATION_HOLD_IDEMPOTENCY_KEY") else {
        return;
    };
    if expected != idempotency_key {
        return;
    }
    let Ok(path) = std::env::var("STORYOS_TEST_CONVERSATION_HOLD_PATH") else {
        return;
    };
    if let Ok(reached) = std::env::var("STORYOS_TEST_CONVERSATION_HOLD_REACHED_PATH") {
        let _write = std::fs::write(reached, "held");
    }
    let path = std::path::PathBuf::from(path);
    while path.exists() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

fn sequence_error(error: CreateAgentRunError) -> ProjectCommandError {
    match error {
        CreateAgentRunError::BindingConflict => ProjectCommandError::BindingConflict,
        CreateAgentRunError::Unavailable(source) => ProjectCommandError::Unavailable(source),
    }
}

fn agent_run_challenge_error(error: ProjectCommandChallengeError) -> CreateAgentRunError {
    CreateAgentRunError::Unavailable(Box::new(error))
}

pub(super) fn agent_run_database_error(error: tokio_postgres::Error) -> CreateAgentRunError {
    CreateAgentRunError::Unavailable(Box::new(error))
}

pub(super) fn agent_run_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> CreateAgentRunError {
    CreateAgentRunError::Unavailable(Box::new(error))
}
