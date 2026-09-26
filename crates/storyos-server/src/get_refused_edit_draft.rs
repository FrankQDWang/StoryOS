use storyos_application::{RefusedEditDraftReader, author_edit_units_to_wire};

use super::*;

pub(super) async fn get_refused_edit_draft(
    State(state): State<Arc<ServerState>>,
    Path((project_id, draft_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetRefusedEditDraftResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&draft_id)?;
    let reader = project_reader(&state).await?;
    let Some(record) = reader
        .read_refused_edit_draft(&scope, &draft_id)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    let digest_hex = record
        .command_digest
        .strip_prefix("sha256:storyos.command.applyAuthorEdit.jcs.v1:")
        .filter(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| {
            service_unavailable(storyos_application::ProjectReadError::unavailable(
                std::io::Error::other("Draft source command digest mismatch"),
            ))
        })?
        .to_owned();
    let payload = record.payload;
    Ok(Json(contracts::GetRefusedEditDraftResponse {
        schema_id: contracts::GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        draft: contracts::RefusedEditDraftInspect {
            draft_id: record.identity.draft_id.clone(),
            draft_revision_id: record.identity.draft_revision_id.clone(),
            kind: "refused_edit".to_owned(),
            closure: record.closure,
            retention_state: record.retention,
            payload: contracts::RefusedEditPayload {
                schema_revision: payload.schema_revision,
                chapter_id: payload.chapter_id,
                expected_authoritative_revision_id: payload.expected_authoritative_revision_id,
                expected_proposal_head_revision_ids: payload.expected_proposal_head_revision_ids,
                target_refs: payload.target_refs,
                author_edit_units: author_edit_units_to_wire(&payload.author_edit_units),
                undo_group_id: payload.undo_group_id,
                completed_intent_record_id: payload.completed_intent_record_id,
                local_intent_sequence: payload.local_intent_sequence,
            },
            payload_digest: record.payload_digest,
            payload_digest_profile: "storyos.refused-edit-payload.jcs.v1".to_owned(),
            creation: contracts::RefusedEditDraftCreated {
                event_kind: "refused_edit_draft_created".to_owned(),
                project_scope: contract_scope(&scope),
                creator: contracts::RefusedEditDraftCreator::CoreTransition {
                    receipt_id: record.source.receipt_id.clone(),
                },
                schema_id: contracts::REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID.to_owned(),
                creation_event_id: record.identity.creation_event_id,
                draft_id: record.identity.draft_id,
                draft_revision_id: record.identity.draft_revision_id,
                created_at: record.created_at,
                source: contracts::RefusedEditDraftSource {
                    command_id: record.source.command_id,
                    author_command_admission_id: record.source.author_command_admission_id,
                    receipt_id: record.source.receipt_id,
                    idempotency_key: record.idempotency_key,
                    command_digest: contracts::DigestValue {
                        algorithm: contracts::DigestAlgorithm::Sha256,
                        profile: "storyos.command.applyAuthorEdit.jcs.v1".to_owned(),
                        value_hex_lowercase: digest_hex,
                    },
                },
            },
        },
    }))
}
