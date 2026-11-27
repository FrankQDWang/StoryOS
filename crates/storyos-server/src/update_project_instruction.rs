use storyos_application::{
    RefusableCommandError, UpdateProjectInstructionInput, open_project_instruction,
};
use storyos_core::{TransitionOutcome, UpdateProjectInstructionRefusal};

use super::command_admission::{
    BodyValidation, ProjectCommandRoute, SettledReceipt, TargetValidation, admit,
    controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

pub(super) async fn get_project_instruction(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetProjectInstructionResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state).await?;
    if open_project(&reader, &scope)
        .await
        .map_err(service_unavailable)?
        .is_none()
    {
        return Err(resource_unavailable());
    }
    let instruction = open_project_instruction(&reader, &scope)
        .await
        .map_err(service_unavailable)?;
    Ok(Json(contracts::GetProjectInstructionResponse {
        schema_id: contracts::GET_PROJECT_INSTRUCTION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        instruction: instruction.map(|revision| contracts::ProjectInstruction {
            instruction_revision_id: revision.instruction_revision_id,
            revision: revision.revision.to_string(),
            text: revision.text,
        }),
    }))
}

const UPDATE_PROJECT_INSTRUCTION: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Update Project Instruction",
    command_kind: contracts::CommandKind::UpdateProjectInstruction,
    method: contracts::UPDATE_PROJECT_INSTRUCTION_METHOD,
    path: contracts::UPDATE_PROJECT_INSTRUCTION_PATH,
    schema_id: contracts::UPDATE_PROJECT_INSTRUCTION_REQUEST_SCHEMA_ID,
    digest_profile: contracts::UPDATE_PROJECT_INSTRUCTION_DIGEST_PROFILE,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
};

pub(super) async fn update_project_instruction(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::UpdateProjectInstructionResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &UPDATE_PROJECT_INSTRUCTION,
        |body: &contracts::UpdateProjectInstructionRequest| {
            let input = &body.update_project_instruction_input;
            // PostgreSQL cannot keep U+0000 in the `jsonb` command bytes or in `text`.
            if input.text.contains('\0') {
                return Err(invalid_request());
            }
            Ok(UpdateProjectInstructionInput {
                text: input.text.clone(),
                expected_revision: input
                    .expected_instruction_revision
                    .parse::<u64>()
                    .map_err(|_| invalid_request())?,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .update_project_instruction(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(
                UpdateProjectInstructionRefusal::InstructionOverItemLimit,
            ) => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "instruction_over_item_limit",
                "The Project Instruction is above the Context item limit.",
            ),
            RefusableCommandError::Command(error) => UPDATE_PROJECT_INSTRUCTION.problem(error),
        })?;
    admitted.hold_first_acknowledgement().await;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            contracts::UpdateProjectInstructionEffect::AuthoritativeApplied {
                instruction_revision_id: applied.effect.instruction_revision_id,
                revision: applied.effect.revision.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => {
            contracts::UpdateProjectInstructionEffect::NoEffect {
                reason: contract_reason(&reason)?,
            }
        }
        TransitionOutcome::Conflicted(reason) => {
            contracts::UpdateProjectInstructionEffect::Conflicted {
                reason: contract_reason(&reason)?,
            }
        }
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &UPDATE_PROJECT_INSTRUCTION,
        contracts::DomainReceiptCommandKind::UpdateProjectInstruction,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::UpdateProjectInstructionResponse {
        schema_id: contracts::UPDATE_PROJECT_INSTRUCTION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
