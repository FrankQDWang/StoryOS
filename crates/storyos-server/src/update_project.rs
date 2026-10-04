use storyos_application::UpdateProjectInput;
use storyos_core::TransitionOutcome;

use super::command_admission::{
    ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch, SettledReceipt, admit,
    positive, structure_title,
};
use super::contract_reason::contract_reason;
use super::*;

const UPDATE_PROJECT: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Update Project",
    command_kind: "updateProject",
    method: contracts::UPDATE_PROJECT_METHOD,
    path: contracts::UPDATE_PROJECT_PATH,
    schema_id: contracts::UPDATE_PROJECT_REQUEST_SCHEMA_ID,
    digest_profile: contracts::UPDATE_PROJECT_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::UpdateProject,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn update_project(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::UpdateProjectResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &UPDATE_PROJECT,
        |body: &contracts::UpdateProjectRequest| {
            let input = &body.update_project_input;
            Ok(UpdateProjectInput {
                expected_revision: positive(&input.expected_project_revision)?,
                title: structure_title(&input.title)?,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .update_project(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| UPDATE_PROJECT.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            contracts::UpdateProjectEffect::AuthoritativeApplied {
                title: applied.effect.title,
                revision: applied.effect.revision.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::UpdateProjectEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::UpdateProjectEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &UPDATE_PROJECT,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            project: settlement.response_project,
        },
    );
    Ok(Json(contracts::UpdateProjectResponse {
        schema_id: contracts::UPDATE_PROJECT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
