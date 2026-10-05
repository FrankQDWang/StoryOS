use storyos_application::{
    ProjectAssistanceRecord, UpdateProjectAssistanceInput, open_project_assistance,
};
use storyos_core::{AssistanceAvailability, TransitionOutcome, UpdateProjectAssistanceApplied};

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

pub(super) async fn get_project_assistance(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetProjectAssistanceResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state).await?;
    let Some(assistance) = open_project_assistance(&reader, &scope)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetProjectAssistanceResponse {
        schema_id: contracts::GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        assistance: contract_assistance(&assistance),
    }))
}

const UPDATE_PROJECT_ASSISTANCE: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Update Project Assistance",
    command_kind: "updateProjectAssistance",
    method: contracts::UPDATE_PROJECT_ASSISTANCE_METHOD,
    path: contracts::UPDATE_PROJECT_ASSISTANCE_PATH,
    schema_id: contracts::UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
    digest_profile: contracts::UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::UpdateProjectAssistance,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn update_project_assistance(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::UpdateProjectAssistanceResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &UPDATE_PROJECT_ASSISTANCE,
        |body: &contracts::UpdateProjectAssistanceRequest| {
            let input = &body.update_project_assistance_input;
            let expected_revision = input
                .expected_assistance_revision
                .parse::<u64>()
                .map_err(|_| invalid_request())?;
            Ok(UpdateProjectAssistanceInput {
                availability: match input.availability {
                    contracts::ProjectAssistanceAvailability::Available => {
                        AssistanceAvailability::Available
                    }
                    contracts::ProjectAssistanceAvailability::Unavailable => {
                        AssistanceAvailability::Unavailable
                    }
                },
                expected_revision,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .update_project_assistance(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| UPDATE_PROJECT_ASSISTANCE.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            let project_activity_position = applied.project_activity_position.to_string();
            match applied.effect {
                UpdateProjectAssistanceApplied::Initialized {
                    availability,
                    revision,
                } => contracts::UpdateProjectAssistanceEffect::Initialized {
                    availability: contract_availability(availability),
                    revision: revision.to_string(),
                    project_activity_position,
                },
                UpdateProjectAssistanceApplied::Changed {
                    availability,
                    revision,
                } => contracts::UpdateProjectAssistanceEffect::AuthoritativeApplied {
                    availability: contract_availability(availability),
                    revision: revision.to_string(),
                    project_activity_position,
                },
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::UpdateProjectAssistanceEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => {
            contracts::UpdateProjectAssistanceEffect::Conflicted {
                reason: contract_reason(&reason)?,
            }
        }
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let Some(assistance) = settlement.response.assistance else {
        return Err(problem(
            StatusCode::CONFLICT,
            "stale_assistance_revision",
            "The Project assistance revision is stale.",
        ));
    };
    let ack = admitted.acknowledgement(
        &UPDATE_PROJECT_ASSISTANCE,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::UpdateProjectAssistanceResponse {
        schema_id: contracts::UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response.project),
        assistance: contract_assistance(&assistance),
        effect,
    }))
}

fn contract_assistance(record: &ProjectAssistanceRecord) -> contracts::ProjectAssistanceBinding {
    contracts::ProjectAssistanceBinding {
        availability: contract_availability(record.availability),
        revision: record.revision.to_string(),
        model_registration_revision: record.model_registration_revision.clone(),
        processing_destination_identity: record.processing_destination_identity.clone(),
        processing_destination_identity_evidence_revision: record
            .processing_destination_identity_evidence_revision
            .to_string(),
        project_model_use_binding_revision: record.project_model_use_binding_revision.clone(),
        external_compatibility_decision: record.external_compatibility_decision.clone(),
    }
}

fn contract_availability(
    availability: AssistanceAvailability,
) -> contracts::ProjectAssistanceAvailability {
    match availability {
        AssistanceAvailability::Available => contracts::ProjectAssistanceAvailability::Available,
        AssistanceAvailability::Unavailable => {
            contracts::ProjectAssistanceAvailability::Unavailable
        }
    }
}
