use storyos_application::SetProjectToolEnablementInput;
use storyos_core::{RESEARCH_TOOL_OFFER, ResearchToolOffer, ToolEnablement, TransitionOutcome};

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const SET_PROJECT_TOOL_ENABLEMENT: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Set Project Tool Enablement",
    command_kind: contracts::CommandKind::SetProjectToolEnablement,
    method: contracts::SET_PROJECT_TOOL_ENABLEMENT_METHOD,
    path: contracts::SET_PROJECT_TOOL_ENABLEMENT_PATH,
    schema_id: contracts::SET_PROJECT_TOOL_ENABLEMENT_REQUEST_SCHEMA_ID,
    digest_profile: contracts::SET_PROJECT_TOOL_ENABLEMENT_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

/// The research destination that the deployment offers: each deployment with a model
/// destination offers it (ADR 0083). The Server never reads its key.
pub(super) fn research_tool_offer(state: &ServerState) -> Option<&'static ResearchToolOffer> {
    state
        .config
        .model_destination
        .as_ref()
        .map(|_| &RESEARCH_TOOL_OFFER)
}

pub(super) async fn set_project_tool_enablement(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::SetProjectToolEnablementResponse>, ApiError> {
    let offer = research_tool_offer(&state);
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &SET_PROJECT_TOOL_ENABLEMENT,
        |body: &contracts::SetProjectToolEnablementRequest| {
            let input = &body.set_project_tool_enablement_input;
            if !(1..=128).contains(&input.registration_id.len()) {
                return Err(invalid_request());
            }
            Ok(SetProjectToolEnablementInput {
                registration_id: input.registration_id.clone(),
                requested: if input.enabled {
                    ToolEnablement::Enabled
                } else {
                    ToolEnablement::Disabled
                },
                offer,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .set_project_tool_enablement(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| SET_PROJECT_TOOL_ENABLEMENT.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            contracts::SetProjectToolEnablementEffect::AuthoritativeApplied {
                enabled: applied.effect.enablement == ToolEnablement::Enabled,
                revision: applied.effect.revision.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => {
            contracts::SetProjectToolEnablementEffect::NoEffect {
                reason: contract_reason(&reason)?,
            }
        }
        TransitionOutcome::Refused(reason) => contracts::SetProjectToolEnablementEffect::Refused {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &SET_PROJECT_TOOL_ENABLEMENT,
        contracts::DomainReceiptCommandKind::SetProjectToolEnablement,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::SetProjectToolEnablementResponse {
        schema_id: contracts::SET_PROJECT_TOOL_ENABLEMENT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
