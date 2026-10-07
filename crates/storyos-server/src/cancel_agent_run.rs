use storyos_application::{AgentRunControlRefusal, CancelAgentRunInput, RefusableCommandError};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const CANCEL_AGENT_RUN: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "AgentRun control",
    command_kind: "cancelAgentRun",
    method: contracts::CANCEL_AGENT_RUN_METHOD,
    path: contracts::CANCEL_AGENT_RUN_PATH,
    schema_id: contracts::CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
    digest_profile: contracts::CANCEL_AGENT_RUN_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn cancel_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CancelAgentRunResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&run_id],
        request,
        &CANCEL_AGENT_RUN,
        |_body: &contracts::CancelAgentRunRequest| {
            Ok(CancelAgentRunInput {
                run_id: run_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .cancel_agent_run(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(AgentRunControlRefusal::MissingRun) => {
                resource_unavailable()
            }
            RefusableCommandError::Command(error) => CANCEL_AGENT_RUN.problem(error),
        })?;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => contracts::CancelAgentRunEffect::Applied {
            run_id: applied.effect.run_id,
            status: contracts::AgentRunStatus::Cancelled,
            fence_generation: applied.effect.fence_generation.to_string(),
            project_activity_position: applied.project_activity_position.to_string(),
        },
        TransitionOutcome::NoEffect(reason) => contracts::CancelAgentRunEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::CancelAgentRunEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &CANCEL_AGENT_RUN,
        contracts::DomainReceiptCommandKind::CancelAgentRun,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::CancelAgentRunResponse {
        schema_id: contracts::CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
