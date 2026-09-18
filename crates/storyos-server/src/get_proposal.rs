use storyos_application::open_proposal;

use super::*;

pub(super) async fn get_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetProposalResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&proposal_id)?;
    let reader = project_reader(&state).await?;
    let Some(record) = open_proposal(&reader, &scope, &proposal_id)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetProposalResponse {
        schema_id: contracts::GET_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        proposal: contracts::BlockProposalInspect {
            proposal_id: record.proposal_id,
            kind: record.kind,
            revision_id: record.revision_id,
            generation: record.generation,
            validation: record.validation,
            condition_refs: record.condition_refs,
            latest_acceptance_refusal: match record.latest_acceptance_refusal {
                None => contracts::OptionalAcceptanceRefusalInspect::Absent,
                Some(refusal) => contracts::OptionalAcceptanceRefusalInspect::Present {
                    refusal_id: refusal.refusal_id,
                    correlation_id: refusal.correlation_id,
                    reason: match refusal.reason {
                        storyos_application::AcceptanceRefusalReason::StaleWriter => {
                            contracts::AcceptanceRefusalReason::StaleWriter
                        }
                        storyos_application::AcceptanceRefusalReason::SessionChanged => {
                            contracts::AcceptanceRefusalReason::SessionChanged
                        }
                        storyos_application::AcceptanceRefusalReason::InvalidChallenge => {
                            contracts::AcceptanceRefusalReason::InvalidChallenge
                        }
                    },
                    boundary: match refusal.boundary {
                        storyos_application::AcceptanceRefusalBoundary::Challenge => {
                            contracts::AcceptanceRefusalBoundary::Challenge
                        }
                        storyos_application::AcceptanceRefusalBoundary::WriterSession => {
                            contracts::AcceptanceRefusalBoundary::WriterSession
                        }
                    },
                    command_schema: refusal.command_schema,
                    refusal_profile_revision: refusal.refusal_profile_revision,
                    client_contract_revision: refusal.client_contract_revision,
                    security_policy_revision: refusal.security_policy_revision,
                    limit_profile_revision: refusal.limit_profile_revision,
                    challenge_rate_policy_revision: refusal.challenge_rate_policy_revision,
                    recorded_at: refusal.recorded_at,
                },
            },
            closure: record.closure,
            operation_id: record.operation_id,
            operation_resolution: record.operation_resolution,
            chapter_id: record.chapter_id,
            manuscript_block_id: record.manuscript_block_id,
            base_authoritative_revision_id: record.base_authoritative_revision_id,
            reservation_state: record.reservation_state,
            candidate_text: record.candidate_text,
            source: contracts::ProposalSourceInspect::AgentRunDecision {
                run_id: record.source_run_id,
                decision_id: record.source_decision_id,
            },
            validation_receipt: match (
                record.validation_receipt_id,
                record.validation_receipt_result,
            ) {
                (Some(validation_receipt_id), Some(result)) => {
                    contracts::OptionalValidationReceiptInspect::Present {
                        validation_receipt_id,
                        result,
                    }
                }
                _ => contracts::OptionalValidationReceiptInspect::Absent,
            },
        },
    }))
}
