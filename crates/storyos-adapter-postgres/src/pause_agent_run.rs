use std::convert::Infallible;

use storyos_application::{
    AgentRunControlRefusal, PauseAgentRunApplied, PauseAgentRunError, PauseAgentRunInput,
    PauseAgentRunSettlement, ProjectCommandEnvelope, ProjectCommandError, RefusableCommandError,
};
use storyos_core::{
    AgentRunLifecycle, PauseAgentRunConflict, PauseAgentRunNoEffect, classify_pause_agent_run,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedResult, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, ProjectActionClass,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect, ZeroReceipt,
    settle_project_command, unavailable,
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
    const SPEC: CommandSpec = CommandSpec {
        kind: "pauseAgentRun",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "agent_run_paused",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Error = PauseAgentRunError;
    type Profile = ActivityOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = ();
    type Plan = ();
    type Effect = PauseAgentRunApplied;
    type NoEffect = PauseAgentRunNoEffect;
    type Conflict = PauseAgentRunConflict;
    type Refusal = Infallible;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, PauseAgentRunError> {
        let status = client
            .query_opt(
                "SELECT status FROM storyos.agent_runs
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                  FOR UPDATE",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &self.run_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .ok_or(RefusableCommandError::RefusedBeforeAdmission(
                AgentRunControlRefusal::MissingRun,
            ))?
            .get::<_, String>(/*idx*/ 0);
        let lifecycle = AgentRunLifecycle::parse(&status)
            .ok_or_else(|| unavailable(format!("unsupported AgentRun status {status}")))?;
        Ok(Classification {
            outcome: classify_pause_agent_run(lifecycle).map_applied(|()| ((), ())),
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
    ) -> Result<ActivityWrite<PauseAgentRunApplied>, ProjectCommandError> {
        let fence_generation = client
            .query_one(
                "UPDATE storyos.agent_runs
                    SET status = 'paused',
                        fence_token = fence_token + 1,
                        lease_expires_at = NULL,
                        wakeup_pending = false
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
              RETURNING fence_token",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &self.run_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .get::<_, i64>(/*idx*/ 0);
        let fence_generation = u64::try_from(fence_generation).map_err(unavailable)?;
        Ok(ActivityWrite {
            activity: serde_json::json!({
                "run_id": self.run_id,
                "fence_generation": fence_generation.to_string(),
            }),
            effect: PauseAgentRunApplied {
                run_id: self.run_id.clone(),
                fence_generation,
            },
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<PauseAgentRunApplied, ReplayFault> {
        Ok(PauseAgentRunApplied {
            run_id: replay.activity_uuid("run_id")?,
            fence_generation: replay.activity_u64("fence_generation")?,
        })
    }
}
