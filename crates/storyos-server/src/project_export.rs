use storyos_application::{
    ArchiveExportRefusal, ExportProjectArchiveInput, GetExportOperation,
    PROJECT_ARCHIVE_ZIP_MEDIA_TYPE, PROJECT_EXPORT_ARCHIVE_PATH_PROFILE,
    PROJECT_EXPORT_ARCHIVE_PROFILE, RefusableCommandError, VerifiedExportArchive,
    get_export_operation, get_verified_export_archive,
};
use storyos_core::ExportProjectArchiveRefusal;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::*;

const EXPORT_PROJECT_ARCHIVE: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Project Export Archive",
    command_kind: "exportProjectArchive",
    method: contracts::EXPORT_PROJECT_ARCHIVE_METHOD,
    path: contracts::EXPORT_PROJECT_ARCHIVE_PATH,
    schema_id: contracts::EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID,
    digest_profile: contracts::EXPORT_PROJECT_ARCHIVE_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn export_project_archive(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<(StatusCode, Json<contracts::ExportProjectArchiveResponse>), ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &EXPORT_PROJECT_ARCHIVE,
        |body: &contracts::ExportProjectArchiveRequest| {
            let input = &body.export_project_archive_input;
            if input.archive_profile != PROJECT_EXPORT_ARCHIVE_PROFILE
                || input.archive_path_profile != PROJECT_EXPORT_ARCHIVE_PATH_PROFILE
            {
                return Err(invalid_request());
            }
            Ok(ExportProjectArchiveInput {
                export_id: Uuid::now_v7().to_string(),
            })
        },
    )
    .await?;
    let admission = admitted
        .store
        .export_project_archive(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(ArchiveExportRefusal::Lifecycle(
                ExportProjectArchiveRefusal::ArchivedProject,
            )) => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "archived_project",
                "The Project is archived.",
            ),
            RefusableCommandError::RefusedBeforeAdmission(ArchiveExportRefusal::Lifecycle(
                ExportProjectArchiveRefusal::MissingProject,
            )) => resource_unavailable(),
            RefusableCommandError::RefusedBeforeAdmission(ArchiveExportRefusal::ArchiveBuild(
                reason,
            )) => archive_build_problem(reason),
            RefusableCommandError::Command(error) => EXPORT_PROJECT_ARCHIVE.problem(error),
        })?;
    let scope = &admitted.envelope.project_scope;
    let operation = admission.work;
    Ok((
        StatusCode::ACCEPTED,
        Json(contracts::ExportProjectArchiveResponse {
            schema_id: contracts::EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID.to_owned(),
            correlation_id: admitted.envelope.correlation_id.clone(),
            project_scope: contract_scope(scope),
            command_id: admission.command_id,
            author_command_admission_id: admission.author_command_admission_id,
            acknowledgement: contracts::ExportAcknowledgement::Accepted,
            operation_ref: Some(contracts::ProjectExportRef::ProjectExport {
                export_id: operation.export_id.clone(),
            }),
            project: controlled_project(admission.response),
            effect: contracts::ExportProjectArchiveEffect::Admitted {
                export_id: operation.export_id,
                archive_profile: operation.archive_profile,
                archive_path_profile: operation.archive_path_profile,
                source_snapshot: Box::new(snapshot_descriptor(scope, &operation.source_snapshot)),
            },
        }),
    ))
}

pub(super) async fn get_export_operation_query(
    State(state): State<Arc<ServerState>>,
    Path((project_id, export_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&export_id)?;
    let reader = project_reader(&state).await?;
    if wants_project_archive_zip(&headers) {
        return match get_verified_export_archive(&reader, &scope, &export_id)
            .await
            .map_err(service_unavailable)?
        {
            VerifiedExportArchive::Missing => Err(resource_unavailable()),
            VerifiedExportArchive::Archived => Err(problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "archived_project",
                "The Project is archived.",
            )),
            VerifiedExportArchive::Expired => Err(problem(
                StatusCode::CONFLICT,
                "snapshot_expired",
                "The Snapshot is no longer available.",
            )),
            VerifiedExportArchive::Unsettled => Err(problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_provenance",
                "The Project Export Archive is not settled.",
            )),
            VerifiedExportArchive::Refused(reason) => Err(archive_build_problem(reason)),
            VerifiedExportArchive::Ready(bytes) => Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, PROJECT_ARCHIVE_ZIP_MEDIA_TYPE),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"storyos-project-export.zip\"",
                    ),
                ],
                bytes,
            )
                .into_response()),
        };
    }
    match get_export_operation(&reader, &scope, &export_id)
        .await
        .map_err(service_unavailable)?
    {
        GetExportOperation::Missing => Err(resource_unavailable()),
        GetExportOperation::Archived => Err(problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "archived_project",
            "The Project is archived.",
        )),
        GetExportOperation::Expired => Err(problem(
            StatusCode::CONFLICT,
            "snapshot_expired",
            "The Snapshot is no longer available.",
        )),
        GetExportOperation::InProgress(progress) => {
            let progress = *progress;
            Ok(Json(contracts::GetExportOperationResponse::InProgress {
                schema_id: contracts::GET_EXPORT_OPERATION_RESPONSE_SCHEMA_ID.to_owned(),
                query_id: Uuid::now_v7().to_string(),
                correlation_id: Uuid::now_v7().to_string(),
                project_scope: contract_scope(&scope),
                export_id: progress.export_id,
                archive_profile: progress.archive_profile,
                archive_path_profile: progress.archive_path_profile,
                source_snapshot: snapshot_descriptor(&scope, &progress.source_snapshot),
            })
            .into_response())
        }
        GetExportOperation::Ready(page) => {
            let page = *page;
            Ok(Json(contracts::GetExportOperationResponse::Ready {
                schema_id: contracts::GET_EXPORT_OPERATION_RESPONSE_SCHEMA_ID.to_owned(),
                query_id: Uuid::now_v7().to_string(),
                correlation_id: Uuid::now_v7().to_string(),
                project_scope: contract_scope(&scope),
                export_id: page.export_id,
                archive_profile: page.archive_profile,
                archive_path_profile: page.archive_path_profile,
                immutable_root: page.immutable_root,
                source_snapshot: snapshot_descriptor(&scope, &page.source_snapshot),
            })
            .into_response())
        }
        GetExportOperation::Failed(progress) => {
            let progress = *progress;
            Ok(Json(contracts::GetExportOperationResponse::Failed {
                schema_id: contracts::GET_EXPORT_OPERATION_RESPONSE_SCHEMA_ID.to_owned(),
                query_id: Uuid::now_v7().to_string(),
                correlation_id: Uuid::now_v7().to_string(),
                project_scope: contract_scope(&scope),
                export_id: progress.export_id,
                archive_profile: progress.archive_profile,
                archive_path_profile: progress.archive_path_profile,
                source_snapshot: snapshot_descriptor(&scope, &progress.source_snapshot),
            })
            .into_response())
        }
        GetExportOperation::OutcomeUnknown(progress) => {
            let progress = *progress;
            Ok(Json(contracts::GetExportOperationResponse::OutcomeUnknown {
                schema_id: contracts::GET_EXPORT_OPERATION_RESPONSE_SCHEMA_ID.to_owned(),
                query_id: Uuid::now_v7().to_string(),
                correlation_id: Uuid::now_v7().to_string(),
                project_scope: contract_scope(&scope),
                export_id: progress.export_id,
                archive_profile: progress.archive_profile,
                archive_path_profile: progress.archive_path_profile,
                source_snapshot: snapshot_descriptor(&scope, &progress.source_snapshot),
            })
            .into_response())
        }
    }
}

fn wants_project_archive_zip(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains("application/vnd.storyos.project-archive+zip"))
}

fn snapshot_descriptor(
    scope: &ApplicationScope,
    snapshot: &storyos_application::CanonicalSnapshot,
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

fn archive_build_problem(reason: storyos_core::ProjectArchiveBuildRefusal) -> ApiError {
    problem(
        StatusCode::UNPROCESSABLE_ENTITY,
        match reason {
            storyos_core::ProjectArchiveBuildRefusal::ForeignMaterial => "foreign_material",
            storyos_core::ProjectArchiveBuildRefusal::CorruptDigest => "corrupt_digest",
            storyos_core::ProjectArchiveBuildRefusal::MissingFamily => "missing_family",
            storyos_core::ProjectArchiveBuildRefusal::IneligibleLifecycle => "ineligible_lifecycle",
            storyos_core::ProjectArchiveBuildRefusal::InvalidProvenance => "invalid_provenance",
            storyos_core::ProjectArchiveBuildRefusal::Collision
            | storyos_core::ProjectArchiveBuildRefusal::DirectoryPrefixCollision
            | storyos_core::ProjectArchiveBuildRefusal::InvalidPath(_) => "archive_path_refused",
        },
        "The Project Export Archive did not complete.",
    )
}
