use storyos_application::{
    CanonicalSnapshot, ExportHumanReadableManuscriptInput, GetHumanReadableManuscriptExport,
    RefusableCommandError, get_human_readable_manuscript_export,
};
use storyos_core::{ExportHumanReadableManuscriptRefusal, READABLE_EXPORT_PROFILE};

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::*;

const EXPORT_HUMAN_READABLE_MANUSCRIPT: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "human-readable export",
    command_kind: "exportHumanReadableManuscript",
    method: contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_METHOD,
    path: contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_PATH,
    schema_id: contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID,
    digest_profile: contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn export_human_readable_manuscript(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<
    (
        StatusCode,
        Json<contracts::ExportHumanReadableManuscriptResponse>,
    ),
    ApiError,
> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &EXPORT_HUMAN_READABLE_MANUSCRIPT,
        |_body: &contracts::ExportHumanReadableManuscriptRequest| {
            Ok(ExportHumanReadableManuscriptInput {
                export_id: Uuid::now_v7().to_string(),
            })
        },
    )
    .await?;
    let admission = admitted
        .store
        .export_human_readable_manuscript(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(
                ExportHumanReadableManuscriptRefusal::ArchivedProject,
            ) => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "archived_project",
                "The Project is archived.",
            ),
            RefusableCommandError::RefusedBeforeAdmission(
                ExportHumanReadableManuscriptRefusal::MissingProject,
            ) => resource_unavailable(),
            RefusableCommandError::Command(error) => {
                EXPORT_HUMAN_READABLE_MANUSCRIPT.problem(error)
            }
        })?;
    let scope = &admitted.envelope.project_scope;
    let export_id = admission.work.export_id;
    Ok((
        StatusCode::ACCEPTED,
        Json(contracts::ExportHumanReadableManuscriptResponse {
            schema_id: contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID.to_owned(),
            correlation_id: admitted.envelope.correlation_id.clone(),
            project_scope: contract_scope(scope),
            command_id: admission.command_id,
            author_command_admission_id: admission.author_command_admission_id,
            acknowledgement: contracts::ExportAcknowledgement::Accepted,
            operation_ref: Some(
                contracts::HumanReadableManuscriptExportRef::HumanReadableManuscriptExport {
                    export_id: export_id.clone(),
                },
            ),
            project: controlled_project(admission.response),
            effect: contracts::ExportHumanReadableManuscriptEffect::Admitted {
                export_id,
                export_profile: READABLE_EXPORT_PROFILE.to_owned(),
                source_snapshot: Box::new(snapshot_descriptor(
                    scope,
                    &admission.work.source_snapshot,
                )),
            },
        }),
    ))
}

pub(super) async fn get_human_readable_manuscript_export_query(
    State(state): State<Arc<ServerState>>,
    Path((project_id, export_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetHumanReadableManuscriptExportResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&export_id)?;
    let reader = project_reader(&state).await?;
    match get_human_readable_manuscript_export(&reader, &scope, &export_id)
        .await
        .map_err(service_unavailable)?
    {
        GetHumanReadableManuscriptExport::Missing => Err(resource_unavailable()),
        GetHumanReadableManuscriptExport::Archived => Err(problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "archived_project",
            "The Project is archived.",
        )),
        GetHumanReadableManuscriptExport::Expired => Err(problem(
            StatusCode::CONFLICT,
            "snapshot_expired",
            "The Snapshot is no longer available.",
        )),
        GetHumanReadableManuscriptExport::InProgress(progress) => {
            let progress = *progress;
            Ok(Json(
                contracts::GetHumanReadableManuscriptExportResponse::InProgress {
                    schema_id: contracts::GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID
                        .to_owned(),
                    query_id: Uuid::now_v7().to_string(),
                    correlation_id: Uuid::now_v7().to_string(),
                    project_scope: contract_scope(&scope),
                    export_id: progress.export_id,
                    export_profile: progress.export_profile,
                    source_snapshot: snapshot_descriptor(&scope, &progress.source_snapshot),
                },
            ))
        }
        GetHumanReadableManuscriptExport::Ready(page) => {
            let page = *page;
            Ok(Json(
                contracts::GetHumanReadableManuscriptExportResponse::Ready {
                    schema_id: contracts::GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID
                        .to_owned(),
                    query_id: Uuid::now_v7().to_string(),
                    correlation_id: Uuid::now_v7().to_string(),
                    project_scope: contract_scope(&scope),
                    export_id: page.export_id,
                    export_profile: page.export_profile,
                    content_sha256: page.content_sha256,
                    manuscript_utf8: page.manuscript_utf8,
                    source_snapshot: snapshot_descriptor(&scope, &page.source_snapshot),
                },
            ))
        }
        GetHumanReadableManuscriptExport::Failed(progress) => {
            let progress = *progress;
            Ok(Json(
                contracts::GetHumanReadableManuscriptExportResponse::Failed {
                    schema_id: contracts::GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID
                        .to_owned(),
                    query_id: Uuid::now_v7().to_string(),
                    correlation_id: Uuid::now_v7().to_string(),
                    project_scope: contract_scope(&scope),
                    export_id: progress.export_id,
                    export_profile: progress.export_profile,
                    source_snapshot: snapshot_descriptor(&scope, &progress.source_snapshot),
                },
            ))
        }
        GetHumanReadableManuscriptExport::OutcomeUnknown(progress) => {
            let progress = *progress;
            Ok(Json(
                contracts::GetHumanReadableManuscriptExportResponse::OutcomeUnknown {
                    schema_id: contracts::GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID
                        .to_owned(),
                    query_id: Uuid::now_v7().to_string(),
                    correlation_id: Uuid::now_v7().to_string(),
                    project_scope: contract_scope(&scope),
                    export_id: progress.export_id,
                    export_profile: progress.export_profile,
                    source_snapshot: snapshot_descriptor(&scope, &progress.source_snapshot),
                },
            ))
        }
    }
}

fn snapshot_descriptor(
    scope: &ApplicationScope,
    snapshot: &CanonicalSnapshot,
) -> contracts::SnapshotDescriptor {
    contracts::SnapshotDescriptor {
        snapshot_id: snapshot.snapshot_id.clone(),
        project_scope: contract_scope(scope),
        snapshot_kind: contracts::SnapshotKind::Canonical,
        project_activity_position: snapshot.project_activity_position.to_string(),
        source_watermarks: contracts::CanonicalSnapshotMaps {},
        projection_generations: contracts::CanonicalSnapshotMaps {},
        redaction_profile: snapshot.redaction_profile.clone(),
        schema_profile: snapshot.schema_profile.clone(),
        replay_generation: snapshot.replay_generation.to_string(),
        created_at: snapshot.created_at.clone(),
        expires_at: snapshot.expires_at.clone(),
    }
}
