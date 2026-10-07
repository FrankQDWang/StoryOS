use storyos_application::{
    AgentRunControlRefusal, ProjectCommandError, RefusableCommandError, SteerAgentRunInput,
};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    AntiForgery, BodyValidation, ProblemMapping, ProjectCommandRequest, ProjectCommandRoute,
    RevisionMismatch, SchemaMismatch, SettledReceipt, TargetValidation, admit_body,
    controlled_project, read_body,
};
use super::contract_reason::contract_reason;
use super::*;

const STEER_AGENT_RUN: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "AgentRun control",
    command_kind: "steerAgentRun",
    method: contracts::STEER_AGENT_RUN_METHOD,
    path: contracts::STEER_AGENT_RUN_PATH,
    schema_id: contracts::STEER_AGENT_RUN_REQUEST_SCHEMA_ID,
    digest_profile: contracts::STEER_AGENT_RUN_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

/// The most characters that one steering message can have.
const STEERING_MESSAGE_CHARACTER_LIMIT: usize = 8000;

impl ProjectCommandRequest for contracts::SteerAgentRunRequest {
    fn command_schema(&self) -> &str {
        &self.command_schema
    }

    fn client_contract_revision(&self) -> &str {
        &self.steer_agent_run_input.client_contract_revision
    }

    fn security_policy_revision(&self) -> &str {
        &self.steer_agent_run_input.security_policy_revision
    }

    fn correlation_id(&self) -> &str {
        &self.steer_agent_run_input.correlation_id
    }
}

pub(super) async fn steer_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::SteerAgentRunResponse>, ApiError> {
    let read = read_body::<contracts::SteerAgentRunRequest>(
        &state,
        &project_id,
        &[&run_id],
        request,
        &STEER_AGENT_RUN,
    )
    .await?;
    // The conversation identity comes before the session in the public problem order.
    valid_uuid(&read.body.steer_agent_run_input.conversation_id)?;
    let admitted = admit_body(
        &state,
        read,
        &STEER_AGENT_RUN,
        AntiForgery::Required,
        |body: &contracts::SteerAgentRunRequest| {
            Ok(SteerAgentRunInput {
                run_id: run_id.clone(),
                conversation_id: body.steer_agent_run_input.conversation_id.clone(),
                author_message: body.steer_agent_run_input.author_message.text.clone(),
            })
        },
    )
    .await?;
    let message = &admitted.input.author_message;
    if message.is_empty() || message.chars().count() > STEERING_MESSAGE_CHARACTER_LIMIT {
        return Err(STEER_AGENT_RUN.problem(ProjectCommandError::BindingConflict));
    }
    let settlement = admitted
        .store
        .steer_agent_run(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(refusal) => control_refusal(refusal),
            RefusableCommandError::Command(error) => STEER_AGENT_RUN.problem(error),
        })?;
    let result = settlement.outcome.receipt_result();
    let effect = match (settlement.outcome, settlement.zero_authority_effect) {
        (TransitionOutcome::NoEffect(_), Some(retained)) => {
            contracts::SteerAgentRunEffect::Retained {
                run_id: retained.run_id,
                steering_input_id: retained.steering_input_id,
                input_position: retained.input_position.to_string(),
            }
        }
        (TransitionOutcome::Conflicted(reason), _) => contracts::SteerAgentRunEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        (TransitionOutcome::NoEffect(_), None) => {
            return Err(STEER_AGENT_RUN.problem(ProjectCommandError::Unavailable(
                "a retained steering input has no effect".into(),
            )));
        }
        (TransitionOutcome::Applied(applied), _) => match applied.effect {},
        (TransitionOutcome::Refused(reason), _) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &STEER_AGENT_RUN,
        contracts::DomainReceiptCommandKind::SteerAgentRun,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::SteerAgentRunResponse {
        schema_id: contracts::STEER_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}

/// The problem of an AgentRun control refusal before Admission.
pub(super) fn control_refusal(refusal: AgentRunControlRefusal) -> ApiError {
    match refusal {
        AgentRunControlRefusal::MissingRun => resource_unavailable(),
        AgentRunControlRefusal::InputLimit => problem(
            StatusCode::PAYLOAD_TOO_LARGE,
            "steering_input_limit",
            "The correction exceeds the admitted Context input limit.",
        ),
    }
}
