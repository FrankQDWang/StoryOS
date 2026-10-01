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
            source_condition: record.source_condition,
            latest_acceptance_refusal: match record.latest_acceptance_refusal {
                None => contracts::OptionalAcceptanceRefusalInspect::Absent,
                Some(refusal) => contracts::OptionalAcceptanceRefusalInspect::Present(Box::new(
                    contracts::AcceptanceRefusalInspect {
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
                )),
            },
            closure: record.closure,
            operation_id: record.operation_id,
            operation_resolution: record.operation_resolution,
            operations: record
                .operations
                .into_iter()
                .map(|operation| contracts::ProposalOperationInspect {
                    candidate_blocks: operation.candidate_blocks,
                    candidate_text: operation.candidate_text,
                    operation_id: operation.operation_id,
                    manuscript_block_id: operation.manuscript_block_id,
                    resolution: operation.resolution,
                    reservation_state: operation.reservation_state,
                })
                .collect(),
            chapter_id: record.chapter_id,
            manuscript_block_id: record.manuscript_block_id,
            base_authoritative_revision_id: record.base_authoritative_revision_id,
            reservation_state: record.reservation_state,
            candidate_text: record.candidate_text,
            candidate_blocks: record.candidate_blocks,
            source: record.source,
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
            anchors: record
                .anchors
                .into_iter()
                .map(|anchor| contracts::ProposalAnchorInspect {
                    manuscript_block_id: anchor.manuscript_block_id,
                    base_authoritative_revision_id: anchor.base_authoritative_revision_id,
                    manuscript_schema_version: anchor.manuscript_schema_version,
                    coordinate_profile: anchor.coordinate_profile,
                    from: anchor.from,
                    to: anchor.to,
                    boundary_profile: anchor.boundary_profile,
                    base_slice_digest: anchor.base_slice_digest,
                })
                .collect(),
            revision_comparison: match record.revision_comparison {
                None => contracts::OptionalRevisionComparisonInspect::Absent,
                Some(comparison) => contracts::OptionalRevisionComparisonInspect::Present {
                    base_authoritative_revision_id: comparison.base_revision_id,
                    candidate_revision_id: comparison.candidate_revision_id,
                    operation_id: comparison.operation_id,
                    spans: comparison
                        .spans
                        .into_iter()
                        .map(|span| contracts::ReplacementSpanInspect {
                            base_from: span.base_from,
                            base_to: span.base_to,
                            candidate_from: span.candidate_from,
                            candidate_to: span.candidate_to,
                            base_text: span.base_text,
                            candidate_text: span.candidate_text,
                        })
                        .collect(),
                },
            },
        },
    }))
}
