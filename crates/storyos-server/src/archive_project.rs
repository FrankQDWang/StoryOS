use storyos_application::ArchiveProjectInput;
use storyos_core::TransitionOutcome;

use super::command_admission::{
    ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch, SettledReceipt, admit,
    positive,
};
use super::contract_reason::contract_reason;
use super::*;

const ARCHIVE_PROJECT: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Archive Project",
    command_kind: "archiveProject",
    method: contracts::ARCHIVE_PROJECT_METHOD,
    path: contracts::ARCHIVE_PROJECT_PATH,
    schema_id: contracts::ARCHIVE_PROJECT_REQUEST_SCHEMA_ID,
    digest_profile: contracts::ARCHIVE_PROJECT_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::ArchiveProject,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn archive_project(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::ArchiveProjectResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &ARCHIVE_PROJECT,
        |body: &contracts::ArchiveProjectRequest| {
            Ok(ArchiveProjectInput {
                expected_revision: positive(&body.archive_project_input.expected_project_revision)?,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .archive_project(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| ARCHIVE_PROJECT.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            contracts::ArchiveProjectEffect::AuthoritativeApplied {
                revision: applied.effect.revision.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::ArchiveProjectEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::ArchiveProjectEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &ARCHIVE_PROJECT,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            project: settlement.response_project,
        },
    );
    Ok(Json(contracts::ArchiveProjectResponse {
        schema_id: contracts::ARCHIVE_PROJECT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
