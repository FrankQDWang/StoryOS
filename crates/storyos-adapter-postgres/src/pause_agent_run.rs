use storyos_contracts::CommandKind;

use storyos_application::{
    PauseAgentRunApplied, PauseAgentRunError, PauseAgentRunInput, PauseAgentRunSettlement,
    ProjectCommandEnvelope, ProjectCommandError,
};
use storyos_core::classify_pause_agent_run;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_control::{FencedStatus, fence_agent_run, lock_agent_run};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedVariant, Classification,
    CommandResponse, CommandSpec, CommandWrite, LockedProject, ProjectActionClass, ProjectCommand,
    ReceiptHeads, ReplayEffect, ZeroReceipt, settle_project_command,
};

impl PostgresProjectReader {
    /// Settles one pauseAgentRun. A missing AgentRun is a refusal before Admission.
    pub async fn pause_agent_run(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &PauseAgentRunInput,
    ) -> Result<PauseAgentRunSettlement, PauseAgentRunError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for PauseAgentRunInput {
    const SPEC: CommandSpec<PauseAgentRunError> = CommandSpec {
        kind: CommandKind::PauseAgentRun,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "agent_run_paused",
        replay_effect: ReplayEffect::NoQuery,
        response: CommandResponse::Project,
        contention_refusal: None,
        receipt_payload: "{}",
    };
    type Error = PauseAgentRunError;
    type Profile = ActivityOnly;
    type Applied = ();
    type Plan = ();
    type Settlement = PauseAgentRunSettlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, PauseAgentRunError> {
        let run = lock_agent_run(client, &envelope.project_scope, &self.run_id).await?;
        Ok(Classification {
            outcome: classify_pause_agent_run(run.lifecycle).map_applied(|()| ((), ())),
            admission: Admission::Project(ProjectActionClass::AgentRunControl),
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
        _plan: (),
        _applied: (),
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        let fence_generation = fence_agent_run(
            client,
            &envelope.project_scope,
            &self.run_id,
            FencedStatus::Paused,
        )
        .await?;
        Ok(CommandWrite {
            profile: ActivityWrite {
                activity: serde_json::json!({
                    "run_id": self.run_id,
                    "fence_generation": fence_generation.to_string(),
                }),
                effect: PauseAgentRunApplied {
                    run_id: self.run_id.clone(),
                    fence_generation,
                },
            },
            after_authority: None,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<PauseAgentRunApplied, ReplayFault> {
        Ok(PauseAgentRunApplied {
            run_id: replay.activity_uuid("run_id")?,
            fence_generation: replay.activity_u64("fence_generation")?,
        })
    }
}
