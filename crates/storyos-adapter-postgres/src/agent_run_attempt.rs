use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, ProjectAssistanceRecord, WirePayloadProjection,
};
use uuid::Uuid;

use crate::agent_run_expiry::RebuildDispatch;
use crate::agent_run_work::{RunPhaseRow, complete_database_error};

/// Commits the dispatch claim: the Model Attempt, its Wire Payload Projection, and its
/// Outbound Disclosure Event, which starts as OutcomeUnknown.
pub(crate) async fn persist_uncertain_attempt(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    assistance: &ProjectAssistanceRecord,
    rebuild: Option<&RebuildDispatch>,
    projection: &WirePayloadProjection,
) -> Result<String, CompleteAgentRunError> {
    let RunPhaseRow {
        author_message,
        chapter_id,
        conversation_id,
        assembly_manifest_id,
        decision_position,
        ..
    } = run;
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
    let mut payload = serde_json::json!({
        "execution_profile": {
            "profile_revision": projection.execution_profile,
            "mapping_revision": projection.mapping_revision,
            "network_io": false,
            "provider_bound": "unknown"
        },
        "wire": {
            "digest": projection.digest,
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
    if let Some(bytes) = &projection.serialized_payload {
        payload["wire"]["serialized_payload"] = serde_json::json!(bytes);
        payload["evidence"][0]["content"] = serde_json::json!(bytes);
    }
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

pub(crate) fn evidence_values(
    attempt_id: &str,
    author_message: &str,
    assembly_manifest_id: &str,
    known_prior_binding_id: Option<&str>,
) -> Vec<serde_json::Value> {
    let mut values = vec![
        serde_json::json!({
            "kind": "sent_content",
            "attempt_id": attempt_id,
            "availability": "current",
            "content": author_message
        }),
        serde_json::json!({
            "kind": "stored_reference",
            "attempt_id": attempt_id,
            "availability": "current",
            "reference_id": assembly_manifest_id
        }),
    ];
    if let Some(reference_id) = known_prior_binding_id {
        values.push(serde_json::json!({
            "kind": "stored_reference",
            "attempt_id": attempt_id,
            "availability": "current",
            "reference_id": reference_id
        }));
    }
    values.push(serde_json::json!({
        "kind": "provider_report",
        "attempt_id": attempt_id,
        "availability": "current",
        "report": "host_fake_no_provider_usage"
    }));
    values.push(serde_json::json!({
        "kind": "provider_opaque",
        "attempt_id": attempt_id,
        "availability": "unknown",
        "unknown_facts": ["provider_internal_content"]
    }));
    values
}
