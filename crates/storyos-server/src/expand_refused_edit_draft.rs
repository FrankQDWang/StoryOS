use super::proposal_generation_decision::{challenge_binding, fresh_ids, prepare_request};
use super::*;
use storyos_application::{
    DraftCloseError, DraftExpansionSettlement, ExpandRefusedEditDraftCommand,
};

pub(super) async fn expand_refused_edit_draft(
    State(state): State<Arc<ServerState>>,
    Path((project_id, draft_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ExpandRefusedEditDraftResponse>, ApiError> {
    let prepared = prepare_request(
        &state,
        &project_id,
        &draft_id,
        request,
        contracts::EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
        contracts::EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE,
        "POST",
        contracts::EXPAND_REFUSED_EDIT_DRAFT_PATH,
        "expandRefusedEditDraftToProposal",
    )
    .await?;
    let body: contracts::ExpandRefusedEditDraftRequest =
        serde_json::from_slice(&prepared.bytes).map_err(|_| invalid_request_shape())?;
    let input = &body.expand_refused_edit_draft_to_proposal_input;
    if body.command_schema != contracts::EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID
        || input.client_contract_revision != prepared.client_binding.client_contract_revision
        || input.security_policy_revision != prepared.client_binding.security_policy_revision
        || input.draft_id != draft_id
        || !input
            .writer_generation
            .parse::<u64>()
            .is_ok_and(|v| v.to_string() == input.writer_generation)
        || input.source_draft_payload_digest.len() != 64
        || !input
            .source_draft_payload_digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid_request());
    }
    for value in [
        &input.editor_session_id,
        &input.correlation_id,
        &input.chapter_id,
        &input.source_current_draft_revision_id,
    ]
    .into_iter()
    .chain(input.target_refs.iter())
    .chain(input.expected_target_revisions.iter())
    .chain(input.source_reopen_event_id.iter())
    {
        valid_uuid(value)?;
    }
    let canonical_command_bytes = storyos_core::canonical_json(
        &serde_json::to_value(&body).map_err(|_| invalid_request_shape())?,
    )
    .into_bytes();
    let mut binding = challenge_binding(
        &prepared,
        &body.command_schema,
        "expandRefusedEditDraftToProposal",
        "POST",
        contracts::EXPAND_REFUSED_EDIT_DRAFT_PATH,
    );
    binding.canonical_command_digest = format!(
        "sha256:{}:{}",
        contracts::EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE,
        storyos_core::hex_sha256(&canonical_command_bytes)
    );
    let command = ExpandRefusedEditDraftCommand {
        project_scope: prepared.scope,
        client_binding: prepared.client_binding,
        challenge_binding: binding,
        nonce_digest: prepared.nonce_digest,
        canonical_command_bytes,
        ids: fresh_ids(),
        draft_id,
        input: body.expand_refused_edit_draft_to_proposal_input,
    };
    let store = project_reader(&state).await?;
    let settled = storyos_application::expand_refused_edit_draft(&store, &command)
        .await
        .map_err(|error| match error {
            DraftCloseError::MissingDraft => resource_unavailable(),
            DraftCloseError::InvalidWriter | DraftCloseError::BindingConflict => problem(
                StatusCode::CONFLICT,
                "draft_expansion_binding_conflict",
                "The Draft expansion binding conflicts.",
            ),
            DraftCloseError::InvalidChallenge => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "challenge_invalid",
                "The Draft expansion challenge is invalid.",
            ),
            DraftCloseError::Unavailable(_) => problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "project_store_unavailable",
                "The Project store is unavailable.",
            ),
        })?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(
        &command.challenge_binding.idempotency_key,
    )
    .await;
    response(&command, settled)
}

fn response(
    command: &ExpandRefusedEditDraftCommand,
    settled: DraftExpansionSettlement,
) -> Result<Json<contracts::ExpandRefusedEditDraftResponse>, ApiError> {
    use contracts::{DomainReceiptResult as ReceiptResult, ExpandRefusedEditDraftEffect as Effect};
    use storyos_core::ExpandRefusedEditDraftResult as ResultKind;
    let scope = contract_scope(&command.project_scope);
    let (result, effect) = match settled.result {
        ResultKind::ProposalCreated { .. } => (
            ReceiptResult::ProposalCreatedFromDraft,
            Effect::ProposalCreatedFromDraft {
                proposal_id: settled.proposal_id.ok_or_else(invalid_request)?,
                proposal_revision_id: settled
                    .proposal_revision_id
                    .clone()
                    .ok_or_else(invalid_request)?,
                event: Box::new(super::close_editor_flow_draft::closed_event(
                    &command.project_scope,
                    &command.draft_id,
                    &settled.draft_revision_id,
                    &settled.payload_digest,
                    &storyos_application::RefusedEditDraftClosure {
                        event_id: settled.event_id.clone().ok_or_else(invalid_request)?,
                        source: settled.ids.clone(),
                        command_digest: command.challenge_binding.canonical_command_digest.clone(),
                        idempotency_key: command.challenge_binding.idempotency_key.clone(),
                        author_action_sequence: settled.author_action_sequence.clone(),
                        close_reason: "superseded".to_owned(),
                        created_at: settled.created_at.clone(),
                    },
                )?),
            },
        ),
        ResultKind::Conflicted => (
            ReceiptResult::Conflicted,
            Effect::Conflicted {
                current_revision_id: settled.draft_revision_id,
                current_digest: settled.payload_digest,
                current_closure: settled.observed_closure,
            },
        ),
        reason @ (ResultKind::SourceDraftNotOpen
        | ResultKind::SourceUnavailable
        | ResultKind::UnsupportedPayload
        | ResultKind::TargetUnavailable) => (
            ReceiptResult::Refused,
            Effect::Refused {
                reason: match reason {
                    ResultKind::SourceDraftNotOpen => "source_draft_not_open",
                    ResultKind::SourceUnavailable => "source_unavailable",
                    ResultKind::UnsupportedPayload => "unsupported_payload",
                    ResultKind::TargetUnavailable => "target_unavailable",
                    ResultKind::Conflicted | ResultKind::ProposalCreated { .. } => unreachable!(),
                }
                .to_owned(),
                current_closure: settled.observed_closure,
            },
        ),
    };
    let heads = settled
        .current_target_revision_id
        .into_iter()
        .collect::<Vec<_>>();
    Ok(Json(contracts::ExpandRefusedEditDraftResponse {
        schema_id: contracts::EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: settled.correlation_id,
        project_scope: scope.clone(),
        command_id: settled.ids.command_id,
        author_command_admission_id: settled.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settled.ids.receipt_id,
            project_scope: scope,
            command_kind: contracts::DomainReceiptCommandKind::ExpandRefusedEditDraftToProposal,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: storyos_core::hex_sha256(&command.canonical_command_bytes),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settled.ids.author_command_admission_id,
            expected_heads: command.input.expected_target_revisions.clone(),
            prior_heads: heads.clone(),
            resulting_heads: heads,
            authoritative_revision_ids: vec![],
            proposal_revision_ids: settled.proposal_revision_id.into_iter().collect(),
            authoritative_commit_ids: vec![],
            author_action_sequence: settled.author_action_sequence,
            draft_artifact_refs: vec![command.draft_id.clone()],
            artifact_lifecycle_event_refs: settled.event_id.into_iter().collect(),
            condition_refs: vec![],
            result,
            created_at: settled.created_at,
        },
        effect,
    }))
}
