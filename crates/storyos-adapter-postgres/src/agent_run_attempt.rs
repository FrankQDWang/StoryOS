use storyos_application::{ClaimedAgentRun, CompleteAgentRunError, ProjectAssistanceRecord};
use storyos_core::{
    HOST_FAKE_EXECUTION_PROFILE, HOST_FAKE_MAPPING_REVISION, host_fake_wire_digest,
};
use uuid::Uuid;

use crate::agent_run_expiry::RebuildDispatch;
use crate::agent_run_work::{complete_database_error, evidence_values};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn persist_uncertain_attempt(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    author_message: &str,
    chapter_id: &str,
    assembly_manifest_id: &str,
    assistance: &ProjectAssistanceRecord,
    rebuild: Option<&RebuildDispatch>,
    decision_position: &str,
    record: &serde_json::Value,
) -> Result<String, CompleteAgentRunError> {
    let model_attempt_id = Uuid::now_v7().to_string();
    let destination_attempt_id = Uuid::now_v7().to_string();
    let outbound_disclosure_event_id = Uuid::now_v7().to_string();
    let destination_context_manifest_id = Uuid::now_v7().to_string();
    let outbound_disclosure_manifest_id = Uuid::now_v7().to_string();
    let wire_payload_projection_id = Uuid::now_v7().to_string();
    let mut continuation = crate::agent_run_continuation::decide_continuation(
        client,
        claim,
        conversation_id,
        author_message,
        assistance,
    )
    .await?;
    let model_invocation_id = match rebuild {
        Some(dispatch) => {
            continuation.mapping = storyos_core::ContinuationInputMapping::Full;
            continuation.prior_binding_id = None;
            dispatch.model_invocation_id.clone()
        }
        None => Uuid::now_v7().to_string(),
    };
    let digest = host_fake_wire_digest(author_message, chapter_id);
    let mut payload = serde_json::json!({
        "execution_profile": {
            "profile_revision": HOST_FAKE_EXECUTION_PROFILE,
            "mapping_revision": HOST_FAKE_MAPPING_REVISION,
            "network_io": false,
            "provider_bound": "unknown"
        },
        "wire": {
            "digest": digest,
            "author_message": author_message,
            "chapter_id": chapter_id,
            "prior_continuation_binding_id": continuation.prior_binding_id
        },
        "items": [],
        "decision": null,
        "usage": { "kind": "unknown" },
        "continuation": crate::agent_run_continuation::encode_wire(&continuation),
        "evidence": evidence_values(
            &model_attempt_id,
            author_message,
            assembly_manifest_id,
            continuation.known_prior_binding_id.as_deref(),
        )
    });
    crate::passage_collection::bind_wire(record, author_message, &mut payload);
    if decision_position == "0"
        && let Some(prepared) = crate::agent_run_successor::prepare_subject(
            author_message,
            &crate::agent_run_successor::SuccessorOrigin {
                owner_user_id: claim.project_scope.owner_user_id.as_ref(),
                project_id: claim.project_scope.project_id.as_ref(),
                conversation_id,
                destination_identity: &assistance.processing_destination_identity,
                predecessor_attempt_id: &model_attempt_id,
                model_invocation_id: &model_invocation_id,
            },
        )
    {
        payload["original_result_retrieval"] = prepared.retrieval;
        payload["unknown_create_successor"] = prepared.marker;
        payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": false});
    } else if decision_position == "0"
        && let Some(subject) = crate::agent_run_retrieval::subject_record(
            author_message,
            claim.project_scope.owner_user_id.as_ref(),
            claim.project_scope.project_id.as_ref(),
            conversation_id,
            &assistance.processing_destination_identity,
        )
    {
        payload["original_result_retrieval"] = subject;
        payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": false});
    }
    client
        .execute(
            "UPDATE storyos.context_assembly_manifests
                SET destination_context_manifest_id = $4::text::uuid,
                    outbound_disclosure_manifest_id = $5::text::uuid
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND context_assembly_manifest_id=$6::text::uuid
                AND destination_context_manifest_id IS NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &destination_context_manifest_id,
                &outbound_disclosure_manifest_id,
                &assembly_manifest_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id,
                destination_attempt_id, outbound_disclosure_event_id,
                destination_context_manifest_id, outbound_disclosure_manifest_id,
                wire_payload_projection_id, model_invocation_id, conversation_id,
                dispatch_state, payload, decision_position)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, 'uncertain',
                     $12::text::jsonb, $13::text::numeric)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &model_attempt_id,
                &destination_attempt_id,
                &outbound_disclosure_event_id,
                &destination_context_manifest_id,
                &outbound_disclosure_manifest_id,
                &wire_payload_projection_id,
                &model_invocation_id,
                &conversation_id,
                &payload.to_string(),
                &decision_position,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(model_attempt_id)
}
