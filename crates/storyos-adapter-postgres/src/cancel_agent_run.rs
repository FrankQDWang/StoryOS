use storyos_contracts::CommandKind;

use storyos_application::{
    CancelAgentRunApplied, CancelAgentRunError, CancelAgentRunInput, CancelAgentRunSettlement,
    ProjectCommandEnvelope, ProjectCommandError, ProjectScope,
};
use storyos_core::classify_cancel_agent_run;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_control::{
    CONTROL_SETTLEMENTS, FencedStatus, fence_agent_run, lock_agent_run,
};
use crate::agent_run_recovery::{in_flight_attempt, mark_cancellation_duties};
use crate::agent_run_successor::prohibit_automatic_successor;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedVariant, Classification,
    CommandResponse, CommandSpec, CommandWrite, LockedProject, ProjectActionClass, ProjectCommand,
    ReceiptHeads, ReplayEffect, ZeroReceipt, settle_project_command_after_contention, unavailable,
};

/// Fences the Run `run_id` as cancelled and does the steps of a cancelled Run: the wakeup for a
/// Model Attempt in flight, no automatic successor, and the fence of each ToolCall that did not
/// end. Returns the fence generation.
async fn cancel_run(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<u64, ProjectCommandError> {
    let in_flight = in_flight_attempt(client, scope, run_id)
        .await
        .map_err(unavailable)?;
    let fence_generation = fence_agent_run(client, scope, run_id, FencedStatus::Cancelled).await?;
    if in_flight {
        mark_cancellation_duties(client, scope, run_id)
            .await
            .map_err(unavailable)?;
    }
    prohibit_automatic_successor(client, scope, run_id)
        .await
        .map_err(unavailable)?;
    crate::agent_run_tool_recovery::fence_on_cancel(client, scope, run_id, fence_generation)
        .await
        .map_err(unavailable)?;
    Ok(fence_generation)
}

/// Each Subrun of the Run `run_id` that works by the rule of Core (`subrun_works`).
async fn working_subruns(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<String>, ProjectCommandError> {
    let rows = client
        .query(
            "SELECT subrun.run_id::text, subrun.status
               FROM storyos.subrun_requests AS request
               JOIN storyos.agent_runs AS subrun
                 ON (subrun.owner_user_id, subrun.project_id, subrun.run_id) =
                    (request.owner_user_id, request.project_id, request.subrun_run_id)
              WHERE request.owner_user_id = $1::text::uuid
                AND request.project_id = $2::text::uuid
                AND request.parent_run_id = $3::text::uuid
              ORDER BY subrun.run_id
                FOR UPDATE OF subrun",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let mut working = Vec::new();
    for row in rows {
        let status = storyos_core::AgentRunStatus::parse(row.get(/*idx*/ 1))
            .ok_or_else(|| unavailable(std::io::Error::other("The Subrun status is damaged")))?;
        if storyos_core::subrun_works(status) {
            working.push(row.get(/*idx*/ 0));
        }
    }
    Ok(working)
}

impl PostgresProjectReader {
    /// Settles one cancelAgentRun. A missing AgentRun is a refusal before Admission.
    pub async fn cancel_agent_run(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &CancelAgentRunInput,
    ) -> Result<CancelAgentRunSettlement, CancelAgentRunError> {
        settle_project_command_after_contention(self, envelope, input, CONTROL_SETTLEMENTS).await
    }
}

impl ProjectCommand for CancelAgentRunInput {
    const SPEC: CommandSpec<CancelAgentRunError> = CommandSpec {
        kind: CommandKind::CancelAgentRun,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "agent_run_cancelled",
        replay_effect: ReplayEffect::NoQuery,
        response: CommandResponse::Project,
        contention_refusal: None,
        receipt_payload: "{}",
    };
    type Error = CancelAgentRunError;
    type Profile = ActivityOnly;
    type Applied = ();
    type Plan = ();
    type Settlement = CancelAgentRunSettlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, CancelAgentRunError> {
        let run = lock_agent_run(client, &envelope.project_scope, &self.run_id).await?;
        Ok(Classification {
            outcome: classify_cancel_agent_run(run.lifecycle).map_applied(|()| ((), ())),
            admission: Admission::Project(ProjectActionClass::AgentRunControl),
            heads: ReceiptHeads::default(),
            zero_receipt: ZeroReceipt::Reason,
        })
    }

    /// Fences the Run and each Subrun of the Run that did not end. Only an in-flight Model Attempt
    /// sets the wakeup, so a new Worker claim sends its one Abort (ADR 0039, ADR 0103 section 3A).
    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        _plan: (),
        _applied: (),
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let fence_generation = cancel_run(client, scope, &self.run_id).await?;
        for subrun in working_subruns(client, scope, &self.run_id).await? {
            cancel_run(client, scope, &subrun).await?;
        }
        Ok(CommandWrite {
            profile: ActivityWrite {
                activity: serde_json::json!({
                    "run_id": self.run_id,
                    "fence_generation": fence_generation.to_string(),
                }),
                effect: CancelAgentRunApplied {
                    run_id: self.run_id.clone(),
                    fence_generation,
                },
            },
            after_authority: None,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<CancelAgentRunApplied, ReplayFault> {
        Ok(CancelAgentRunApplied {
            run_id: replay.activity_uuid("run_id")?,
            fence_generation: replay.activity_u64("fence_generation")?,
        })
    }
}
