use storyos_contracts::CommandKind;

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
    CommandResponse, CommandSpec, CommandWrite, LockedProject, ProjectActionClass, ProjectCommand,
    ReceiptHeads, ReplayEffect, ZeroReceipt, settle_project_command, unavailable,
};
use crate::project_research_binding::{ResearchPlan, insert_research_binding, research_plan};
use crate::update_project_assistance::read_assistance_record;

use super::*;

#[path = "create_agent_run_context.rs"]
pub(crate) mod context;
#[path = "agent_run_earlier_turns.rs"]
mod earlier_turns;
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
    const SPEC: CommandSpec<CreateAgentRunCommandError> = CommandSpec {
        kind: CommandKind::CreateAgentRun,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "agent_run_created",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object('run_id', run_id::text)::text
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND receipt_id = $3::text::uuid",
        ),
        response: CommandResponse::Project,
        contention_refusal: Some(|| {
            RefusableCommandError::RefusedBeforeAdmission(CreateAgentRunRefusal::ConversationBusy)
        }),
        receipt_payload: "{}",
    };
    type Error = CreateAgentRunCommandError;
    type Profile = ActivityOnly;
    type Applied = ();
    type Plan = (
        AssistancePlan,
        ResearchPlan,
        Option<&'static storyos_core::EmbeddingRegistration>,
        Vec<storyos_core::SkillRequirement>,
    );
    type Settlement = CreateAgentRunSettlement;

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
        let mut skill_requirements =
            storyos_core::admit_skill_selections(&self.skills, &self.skill_selections).map_err(
                |storyos_core::SkillSelectionUnavailable| {
                    RefusableCommandError::RefusedBeforeAdmission(
                        CreateAgentRunRefusal::SkillSelectionUnavailable,
                    )
                },
            )?;
        if let ConversationSelection::Existing { conversation_id } = &self.conversation {
            let earlier = earlier_skill_requirements(client, scope, conversation_id).await?;
            skill_requirements.extend(storyos_core::conversation_skill_requirements(
                &earlier,
                &skill_requirements,
            ));
        }
        let destination =
            self.destination
                .clone()
                .ok_or(RefusableCommandError::RefusedBeforeAdmission(
                    CreateAgentRunRefusal::AssistanceUnavailable,
                ))?;
        let offered =
            crate::project_destination_binding::current_binding_offers(client, scope, &destination)
                .await
                .map_err(unavailable)?;
        let assistance = match assistance {
            Some(record) if offered => AssistancePlan::Current(record),
            Some(record) => AssistancePlan::Bind {
                revision: record.revision + 1,
                destination,
            },
            None => AssistancePlan::Bind {
                revision: 1,
                destination,
            },
        };
        let research = research_plan(client, scope, self.research).await?;
        let embedding =
            crate::project_embedding_binding::embedding_plan(client, scope, self.embedding).await?;
        Ok(Classification {
            outcome: outcome
                .map_applied(|()| ((), (assistance, research, embedding, skill_requirements))),
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
        (assistance, research, embedding, skill_requirements): (
            AssistancePlan,
            ResearchPlan,
            Option<&'static storyos_core::EmbeddingRegistration>,
            Vec<storyos_core::SkillRequirement>,
        ),
        _applied: (),
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        if let ResearchPlan::Bind(offer) = research {
            insert_research_binding(client, envelope, offer)
                .await
                .map_err(unavailable)?;
        }
        if let Some(registration) = embedding {
            crate::project_embedding_binding::insert_embedding_binding(
                client,
                &envelope.project_scope,
                registration,
            )
            .await
            .map_err(unavailable)?;
        }
        let assistance = match assistance {
            AssistancePlan::Current(record) => record,
            AssistancePlan::Bind {
                revision,
                destination,
            } => bind_deployment_destination(client, envelope, &destination, revision).await?,
        };
        let instruction = project_instruction_binding(client, envelope).await?;
        let applied = write::persist_conversation_and_run(
            client,
            envelope,
            self,
            &assistance.grant_id,
            &assistance.project_model_use_binding_revision,
            instruction.as_deref(),
        )
        .await
        .map_err(sequence_error)?;
        crate::skill_package_snapshot::pin_skill_catalog(
            client,
            envelope,
            &applied.run_id,
            &self.skills,
        )
        .await?;
        let memory_revision = crate::memory_entries::memory_revision_pin(
            client,
            &envelope.project_scope,
            &applied.run_id,
        )
        .await
        .map_err(|error| sequence_error(CreateAgentRunError::Unavailable(error)))?;
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
                candidate_target: None,
                selection_references: self.selection_references.as_deref(),
                skill_requirements: &skill_requirements,
                memory_revision,
                tool_results: None,
            },
            &assistance.processing_destination_identity,
        )
        .await
        .map_err(sequence_error)?;
        Ok(CommandWrite {
            profile: ActivityWrite {
                activity: serde_json::json!({
                    "project_agent_id": applied.project_agent_id,
                    "conversation_id": applied.conversation_id,
                    "memory_settings_revision": applied.memory_settings_revision,
                    "run_id": applied.run_id,
                }),
                effect: applied,
            },
            after_authority: None,
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
    /// The Project has no binding, or its binding records differ from the deployment destination.
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

/// The Skill requirements of each earlier Run of the Project Conversation, oldest Run first, from
/// the Operation Requirement of its first RunStep (ADR 0089 section 5). Each Run of the table
/// passed Admission. A message that `createAgentRun` refused before Admission has no Run.
async fn earlier_skill_requirements(
    client: &Client,
    scope: &ProjectScope,
    conversation_id: &str,
) -> Result<Vec<Vec<storyos_core::SkillRequirement>>, ProjectCommandError> {
    let rows = client
        .query(
            "SELECT (manifest.payload #> '{operation_requirement,skill_requirements}')::text
               FROM storyos.agent_runs AS run
               JOIN storyos.context_assembly_manifests AS manifest
                 ON (manifest.owner_user_id, manifest.project_id, manifest.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND manifest.manifest_role = 'decision' AND manifest.decision_position = 0
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.conversation_id = $3::text::uuid
                AND manifest.payload #> '{operation_requirement,skill_requirements}' IS NOT NULL
              ORDER BY run.run_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    rows.iter()
        .map(|row| serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).map_err(unavailable))
        .collect()
}

/// The Project Instruction Binding of the new Run: the current Revision of the Project, or
/// `None` for "none configured" (ADR 0094 section 2).
async fn project_instruction_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<Option<String>, ProjectCommandError> {
    let current =
        crate::update_project_instruction::read_current_revision(client, &envelope.project_scope)
            .await
            .map_err(|error| unavailable(format!("{error:?}")))?;
    Ok(storyos_core::project_instruction_binding(
        current
            .map(|revision| storyos_core::BoundInstruction {
                revision_id: revision.instruction_revision_id,
                text: revision.text,
            })
            .as_ref(),
    ))
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
