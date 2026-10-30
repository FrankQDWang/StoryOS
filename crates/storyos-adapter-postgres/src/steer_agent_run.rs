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
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedVariant, Classification,
    CommandSpec, LockedProject, ProjectActionClass, ProjectCommand, ProjectResponse, ReceiptHeads,
    ReplayEffect, ZeroAuthorityRows, ZeroAuthorityWrite, ZeroOutcome, ZeroReceipt,
    settle_project_command, unavailable,
};

const ACTIVITY_KIND: &str = "agent_run_steering_retained";

impl PostgresProjectReader {
    /// Settles one steerAgentRun. A missing AgentRun, another conversation, an input above the
    /// Context item bound, and a Run with more than eight references are refusals before
    /// Admission.
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
        applied: &[AppliedVariant::NoAuthorAction],
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
                "SELECT status, conversation_id::text, author_message,
                        active_decision_position::text
                   FROM storyos.agent_runs
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
            if let Some(added) = &self.selection_references {
                let position = run.get::<_, String>(/*idx*/ 3);
                let current = run_references(client, envelope, &self.run_id, &position).await?;
                let merged = storyos_core::merge_steering_references(current, added);
                if merged.len() > storyos_core::SELECTION_REFERENCE_LIMIT {
                    return Err(RefusableCommandError::RefusedBeforeAdmission(
                        AgentRunControlRefusal::ReferenceLimit,
                    ));
                }
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
        let mut activity = serde_json::json!({
            "kind": ACTIVITY_KIND,
            "run_id": effect.run_id,
            "conversation_id": self.conversation_id,
            "steering_input_id": effect.steering_input_id,
            "input_position": effect.input_position.to_string(),
            "author_message": self.author_message,
        });
        if let Some(references) = &self.selection_references {
            activity["selection_references"] = serde_json::json!(references);
        }
        Ok(ZeroAuthorityWrite { activity, effect })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<SteeringRetained>, ReplayFault> {
        match replay.outcome::<SteerAgentRunNoEffect, SteerAgentRunConflict, Infallible>(
            Self::SPEC.applied,
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

/// The references of the Run at its active decision, then the references of each retained input
/// that the Run has not read yet.
async fn run_references(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    run_id: &str,
    position: &str,
) -> Result<Vec<storyos_core::SelectionReference>, SteerAgentRunError> {
    let scope = &envelope.project_scope;
    let params: [&(dyn tokio_postgres::types::ToSql + Sync); 4] = [
        &scope.owner_user_id.as_ref(),
        &scope.project_id.as_ref(),
        &run_id,
        &position,
    ];
    let requirement = client
        .query_opt(
            "SELECT payload::text FROM storyos.operation_requirements
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND requirement_role = 'primary'
                AND decision_position = $4::text::numeric",
            &params,
        )
        .await
        .map_err(unavailable)?;
    let mut references = match requirement {
        Some(row) => {
            let payload: serde_json::Value =
                serde_json::from_str(&row.get::<_, String>(/*idx*/ 0)).map_err(unavailable)?;
            storyos_core::decode_assembly_record(&payload)
                .ok_or_else(|| unavailable("the active Operation Requirement is unavailable"))?
                .operation_requirement
                .selection_references
                .into_iter()
                .flatten()
                .map(|admitted| admitted.reference)
                .collect()
        }
        None => Vec::new(),
    };
    let retained = client
        .query(
            "SELECT (payload->'selection_references')::text
               FROM storyos.project_activity_event_payloads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND event_kind = 'agent_run_steering_retained' AND payload->>'run_id' = $3
                AND (payload->>'input_position')::numeric > $4::text::numeric
                AND payload ? 'selection_references'
              ORDER BY (payload->>'input_position')::numeric",
            &params,
        )
        .await
        .map_err(unavailable)?;
    for row in retained {
        let added: Vec<storyos_core::SelectionReference> =
            serde_json::from_str(&row.get::<_, String>(/*idx*/ 0)).map_err(unavailable)?;
        references = storyos_core::merge_steering_references(references, &added);
    }
    Ok(references)
}
