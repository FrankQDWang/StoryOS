use std::convert::Infallible;

use storyos_application::{
    AgentRunControlRefusal, ProjectCommandEnvelope, ProjectCommandError, RefusableCommandError,
    SteerAgentRunError, SteerAgentRunInput, SteerAgentRunSettlement, SteeringRetained,
};
use storyos_core::{
    AgentRunLifecycle, CONTEXT_ITEM_TOKEN_LIMIT, SteerAgentRunConflict, SteerAgentRunNoEffect,
    TransitionOutcome, classify_steer_agent_run,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedResult, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, ProjectActionClass,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect,
    ZeroAuthorityRows, ZeroAuthorityWrite, ZeroOutcome, ZeroReceipt, settle_project_command,
    unavailable,
};

const ACTIVITY_KIND: &str = "agent_run_steering_retained";

impl PostgresProjectReader {
    /// Settles one steerAgentRun. A missing AgentRun, another conversation, and an input above
    /// the Context item bound are refusals before Admission.
    pub async fn steer_agent_run(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &SteerAgentRunInput,
    ) -> Result<SteerAgentRunSettlement, SteerAgentRunError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for SteerAgentRunInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "steerAgentRun",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: ACTIVITY_KIND,
        replay_effect: ReplayEffect::NoQuery,
    };
    type Error = SteerAgentRunError;
    type Profile = ActivityOnly;
    type Response = ProjectResponse;
    type ZeroEffect = SteeringRetained;
    type Applied = Infallible;
    type Plan = ();
    type Effect = Infallible;
    type NoEffect = SteerAgentRunNoEffect;
    type Conflict = SteerAgentRunConflict;
    type Refusal = Infallible;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, SteerAgentRunError> {
        let scope = &envelope.project_scope;
        let missing_run =
            || RefusableCommandError::RefusedBeforeAdmission(AgentRunControlRefusal::MissingRun);
        let run = client
            .query_opt(
                "SELECT status, conversation_id::text, author_message FROM storyos.agent_runs
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                  FOR UPDATE",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.run_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .ok_or_else(missing_run)?;
        let status = run.get::<_, String>(/*idx*/ 0);
        let lifecycle = AgentRunLifecycle::parse(&status)
            .ok_or_else(|| unavailable(format!("unsupported AgentRun status {status}")))?;
        if self.conversation_id != run.get::<_, String>(/*idx*/ 1) {
            return Err(missing_run());
        }
        let outcome = classify_steer_agent_run(lifecycle);
        if let TransitionOutcome::NoEffect(SteerAgentRunNoEffect::SteeringRetained) = outcome {
            let retained = client
                .query_one(
                    "SELECT COALESCE(sum(char_length(payload->>'author_message') + 1), 0)::text
                       FROM storyos.project_activity_event_payloads
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND event_kind = 'agent_run_steering_retained'
                        AND payload->>'run_id' = $3",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &self.run_id,
                    ],
                )
                .await
                .map_err(unavailable)?
                .get::<_, String>(/*idx*/ 0)
                .parse::<u64>()
                .map_err(unavailable)?;
            let run_message = run.get::<_, String>(/*idx*/ 2).chars().count() as u64;
            let message = self.author_message.chars().count() as u64;
            if run_message + retained + message + 1 > CONTEXT_ITEM_TOKEN_LIMIT {
                return Err(RefusableCommandError::RefusedBeforeAdmission(
                    AgentRunControlRefusal::InputLimit,
                ));
            }
        }
        Ok(Classification {
            outcome: outcome.map_applied(|applied| match applied {}),
            admission: Admission::Project(ProjectActionClass::AgentRunControl),
            heads: ReceiptHeads::default(),
            zero_receipt: ZeroReceipt::Reason,
        })
    }

    async fn apply(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        _plan: (),
        applied: Infallible,
    ) -> Result<ActivityWrite<Infallible>, ProjectCommandError> {
        match applied {}
    }

    fn decode(&self, _replay: &CommandReplay) -> Result<Infallible, ReplayFault> {
        Err(ReplayFault::BindingConflict)
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        match outcome {
            ZeroOutcome::NoEffect(SteerAgentRunNoEffect::SteeringRetained) => {
                ZeroAuthorityRows::EffectWithActivity
            }
            ZeroOutcome::Conflicted(SteerAgentRunConflict::TerminalRun) => ZeroAuthorityRows::None,
            ZeroOutcome::Refused(reason) => match **reason {},
        }
    }

    /// Retains the input at the next input position and resumes a paused Run. The fence token
    /// does not change, so a Worker claim of the Run stays valid.
    async fn write_zero_authority_activity(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _activity: &ActivitySequences,
    ) -> Result<ZeroAuthorityWrite<SteeringRetained>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let input_position = client
            .query_one(
                "SELECT (COALESCE(max((payload->>'input_position')::numeric), 0) + 1)::text
                   FROM storyos.project_activity_event_payloads
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND event_kind = 'agent_run_steering_retained'
                    AND payload->>'run_id' = $3",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.run_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .get::<_, String>(/*idx*/ 0)
            .parse::<u64>()
            .map_err(unavailable)?;
        client
            .execute(
                "UPDATE storyos.agent_runs SET status = 'queued', wakeup_pending = true
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid AND status = 'paused'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.run_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        let effect = SteeringRetained {
            run_id: self.run_id.clone(),
            steering_input_id: envelope.ids.author_command_admission_id.clone(),
            input_position,
        };
        // The Worker reads these keys, with the input position as decimal text.
        Ok(ZeroAuthorityWrite {
            activity: serde_json::json!({
                "kind": ACTIVITY_KIND,
                "run_id": effect.run_id,
                "conversation_id": self.conversation_id,
                "steering_input_id": effect.steering_input_id,
                "input_position": effect.input_position.to_string(),
                "author_message": self.author_message,
            }),
            effect,
        })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<SteeringRetained>, ReplayFault> {
        match replay.outcome::<SteerAgentRunNoEffect, SteerAgentRunConflict, Infallible>(
            Self::SPEC.applied_result.code(),
        )? {
            TransitionOutcome::NoEffect(SteerAgentRunNoEffect::SteeringRetained) => {
                Ok(Some(SteeringRetained {
                    run_id: replay.activity_uuid("run_id")?,
                    steering_input_id: replay.activity_uuid("steering_input_id")?,
                    input_position: replay.activity_u64("input_position")?,
                }))
            }
            TransitionOutcome::Conflicted(SteerAgentRunConflict::TerminalRun) => Ok(None),
            TransitionOutcome::Applied(()) => Err(ReplayFault::BindingConflict),
            TransitionOutcome::Refused(reason) => match reason {},
        }
    }
}
