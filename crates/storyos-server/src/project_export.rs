use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, ExportProjectArchiveCommand,
    ExportProjectArchiveError, ExportProjectArchiveSettlementEffect, GetExportOperation,
    PROJECT_ARCHIVE_ZIP_MEDIA_TYPE, PROJECT_EXPORT_ARCHIVE_PATH_PROFILE,
    PROJECT_EXPORT_ARCHIVE_PROFILE, ProjectCommandChallengeBinding, VerifiedExportArchive,
    get_export_operation, get_verified_export_archive, request_export_project_archive,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn export_project_archive(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<(StatusCode, Json<contracts::ExportProjectArchiveResponse>), ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::ExportProjectArchiveRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.export_project_archive_input;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
        || input.archive_profile != PROJECT_EXPORT_ARCHIVE_PROFILE
        || input.archive_path_profile != PROJECT_EXPORT_ARCHIVE_PATH_PROFILE
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.correlation_id)?;
    let idempotency_key = exact_header(&headers, "idempotency-key")?;
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(idempotency_key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_request());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, session_handle);
    let canonical_command_bytes = canonical_body_bytes(&body)?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let canonical_command_digest = format!(
        "sha256:{}:{digest_hex}",
        contracts::EXPORT_PROJECT_ARCHIVE_DIGEST_PROFILE
    );
    let store = project_reader(&state)?;
    let settlement = request_export_project_archive(
        &store,
        &ExportProjectArchiveCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: binding_ref.clone(),
                session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
            },
            challenge_binding: ProjectCommandChallengeBinding {
                project_scope: scope.clone(),
                client_session_binding_digest: binding_ref,
                client_session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
                limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
                challenge_rate_policy_revision:
                    storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
                method: contracts::EXPORT_PROJECT_ARCHIVE_METHOD.to_owned(),
                route_template: contracts::EXPORT_PROJECT_ARCHIVE_PATH.to_owned(),
                command_schema: body.command_schema.clone(),
                command_kind: "exportProjectArchive".to_owned(),
                canonical_command_digest: canonical_command_digest.clone(),
                idempotency_key: idempotency_key.to_owned(),
            },
            nonce_digest: plain_digest(nonce.as_bytes()),
            canonical_command_bytes,
            correlation_id: input.correlation_id.clone(),
            ids: AuthorCommandAdmissionIds {
                command_id: Uuid::now_v7().to_string(),
                author_command_admission_id: Uuid::now_v7().to_string(),
                receipt_id: Uuid::now_v7().to_string(),
            },
            export_id: Uuid::now_v7().to_string(),
            archive_profile: input.archive_profile.clone(),
            archive_path_profile: input.archive_path_profile.clone(),
        },
    )
    .await
    .map_err(export_error)?;
    let project = open_project(&store, &scope)
        .await
        .map_err(service_unavailable)?
        .ok_or_else(resource_unavailable)?;
    export_response(&scope, &input.correlation_id, project, settlement)
}

fn export_response(
    scope: &ApplicationScope,
    correlation_id: &str,
    project: storyos_application::Project,
    settlement: storyos_application::ExportProjectArchiveSettlement,
) -> Result<(StatusCode, Json<contracts::ExportProjectArchiveResponse>), ApiError> {
    let ExportProjectArchiveSettlementEffect::Admitted {
        archive_profile,
        archive_path_profile,
        source_snapshot,
    } = settlement.effect;
    Ok((
        StatusCode::ACCEPTED,
        Json(contracts::ExportProjectArchiveResponse {
            schema_id: contracts::EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID.to_owned(),
            correlation_id: correlation_id.to_owned(),
            project_scope: contract_scope(scope),
            command_id: settlement.ids.command_id,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            acknowledgement: contracts::ExportAcknowledgement::Accepted,
            operation_ref: Some(contracts::ProjectExportRef::ProjectExport {
                export_id: settlement.export_id.clone(),
            }),
            project: contracts::ControlledProject {
                project_id: project.project_id.as_ref().to_owned(),
                title: project.title,
                open: match project.current_chapter_id {
                    Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                        current_chapter_id: chapter_id.as_ref().to_owned(),
                    },
                    None => contracts::ProjectOpenState::Empty,
                },
            },
            effect: contracts::ExportProjectArchiveEffect::Admitted {
                export_id: settlement.export_id,
                archive_profile,
                archive_path_profile,
                source_snapshot: Box::new(snapshot_descriptor(scope, &source_snapshot)),
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
    let reader = project_reader(&state)?;
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

fn canonical_body_bytes(
    body: &contracts::ExportProjectArchiveRequest,
) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonical_json(value)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn export_error(error: ExportProjectArchiveError) -> ApiError {
    match error {
        ExportProjectArchiveError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Project Export Archive binding conflicts.",
        ),
        ExportProjectArchiveError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Project Export Archive challenge is invalid.",
        ),
        ExportProjectArchiveError::MissingProject => resource_unavailable(),
        ExportProjectArchiveError::ArchivedProject => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "archived_project",
            "The Project is archived.",
        ),
        ExportProjectArchiveError::ArchiveBuild(reason) => archive_build_problem(reason),
        ExportProjectArchiveError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
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
