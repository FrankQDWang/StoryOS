use storyos_application::{AgentRunControlRefusal, PauseAgentRunInput, RefusableCommandError};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const PAUSE_AGENT_RUN: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "AgentRun control",
    command_kind: "pauseAgentRun",
    method: contracts::PAUSE_AGENT_RUN_METHOD,
    path: contracts::PAUSE_AGENT_RUN_PATH,
    schema_id: contracts::PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
    digest_profile: contracts::PAUSE_AGENT_RUN_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn pause_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::PauseAgentRunResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&run_id],
        request,
        &PAUSE_AGENT_RUN,
        |_body: &contracts::PauseAgentRunRequest| {
            Ok(PauseAgentRunInput {
                run_id: run_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .pause_agent_run(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(AgentRunControlRefusal::MissingRun) => {
                resource_unavailable()
            }
            RefusableCommandError::Command(error) => PAUSE_AGENT_RUN.problem(error),
        })?;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => contracts::PauseAgentRunEffect::Applied {
            run_id: applied.effect.run_id,
            status: contracts::AgentRunStatus::Paused,
            fence_generation: applied.effect.fence_generation.to_string(),
            project_activity_position: applied.project_activity_position.to_string(),
        },
        TransitionOutcome::NoEffect(reason) => contracts::PauseAgentRunEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::PauseAgentRunEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &PAUSE_AGENT_RUN,
        contracts::DomainReceiptCommandKind::PauseAgentRun,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::PauseAgentRunResponse {
        schema_id: contracts::PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
