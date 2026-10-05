use storyos_application::{
    CloseEditorFlowDraftInput, DraftCloseObservation, EditorSessionId, ProjectCommandError,
    ProjectScope,
};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit,
};
use super::contract_reason::contract_reason;
use super::*;

const CLOSE_EDITOR_FLOW_DRAFT: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Draft Discard",
    command_kind: "closeEditorFlowDraft",
    method: "POST",
    path: contracts::CLOSE_EDITOR_FLOW_DRAFT_PATH,
    schema_id: contracts::CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID,
    digest_profile: contracts::CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Route(close_problem),
};

fn close_problem(error: ProjectCommandError) -> ApiError {
    match error {
        ProjectCommandError::WriterIneligible => problem(
            StatusCode::CONFLICT,
            "draft_writer_ineligible",
            "The Draft Discard session is not the current writer.",
        ),
        ProjectCommandError::MissingProject => resource_unavailable(),
        ProjectCommandError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Draft Discard challenge is invalid.",
        ),
        ProjectCommandError::BindingConflict
        | ProjectCommandError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "draft_binding_conflict",
            "The Draft Discard binding conflicts.",
        ),
        ProjectCommandError::Unavailable(_) => store_unavailable(),
    }
}

fn store_unavailable() -> ApiError {
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "project_store_unavailable",
        "The Project store is unavailable.",
    )
}

pub(super) async fn close_editor_flow_draft(
    State(state): State<Arc<ServerState>>,
    Path((project_id, draft_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CloseEditorFlowDraftResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&draft_id],
        request,
        &CLOSE_EDITOR_FLOW_DRAFT,
        |body: &contracts::CloseEditorFlowDraftRequest| {
            let input = &body.close_editor_flow_draft_input;
            let writer_generation = input
                .writer_generation
                .parse::<u64>()
                .ok()
                .filter(|generation| generation.to_string() == input.writer_generation);
            let Some(writer_generation) = writer_generation else {
                return Err(invalid_request());
            };
            if input.draft_id != draft_id
                || input.draft_kind != "refused_edit"
                || input.expected_closure != "open"
                || input.close_reason != "abandoned"
                || input.source_draft_payload_digest.len() != 64
                || !input
                    .source_draft_payload_digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid_request());
            }
            valid_uuid(&input.editor_session_id)?;
            valid_uuid(&input.source_current_draft_revision_id)?;
            if let Some(event_id) = &input.source_reopen_event_id {
                valid_uuid(event_id)?;
            }
            Ok(CloseEditorFlowDraftInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                writer_generation,
                draft_id: draft_id.clone(),
                source_current_draft_revision_id: input.source_current_draft_revision_id.clone(),
                source_draft_payload_digest: input.source_draft_payload_digest.clone(),
                source_reopen_event_id: input.source_reopen_event_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .close_editor_flow_draft(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| CLOSE_EDITOR_FLOW_DRAFT.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let envelope = &admitted.envelope;
    let input = &admitted.input;
    let observed =
        |observation: Option<DraftCloseObservation>| observation.ok_or_else(store_unavailable);
    let (result, author_action_sequence, event_id, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            let author_action_sequence = Some(applied.author_action_sequence.to_string());
            let closed = applied.effect;
            let event = closed_event(
                &envelope.project_scope,
                &input.draft_id,
                &closed.observation.draft_revision_id,
                &closed.observation.payload_digest,
                &storyos_application::RefusedEditDraftClosure {
                    event_id: closed.event_id.clone(),
                    source: settlement.ids.clone(),
                    command_digest: envelope.challenge_binding.canonical_command_digest.clone(),
                    idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
                    author_action_sequence: author_action_sequence.clone(),
                    close_reason: "abandoned".to_owned(),
                    created_at: settlement.receipt_created_at.clone(),
                },
            )?;
            (
                contracts::DomainReceiptResult::DraftClosureChanged,
                author_action_sequence,
                Some(closed.event_id),
                contracts::CloseEditorFlowDraftEffect::DraftClosureChanged {
                    event: Box::new(event),
                },
            )
        }
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(_) => {
            let observation = observed(settlement.zero_authority_effect)?;
            (
                contracts::DomainReceiptResult::Conflicted,
                None,
                None,
                contracts::CloseEditorFlowDraftEffect::Conflicted {
                    current_revision_id: observation.draft_revision_id,
                    current_digest: observation.payload_digest,
                    current_closure: observation.observed_closure,
                },
            )
        }
        TransitionOutcome::Refused(reason) => (
            contracts::DomainReceiptResult::Refused,
            None,
            None,
            contracts::CloseEditorFlowDraftEffect::Refused {
                reason: contract_reason(&reason)?,
                current_closure: observed(settlement.zero_authority_effect)?.observed_closure,
            },
        ),
    };
    let project_scope = contract_scope(&envelope.project_scope);
    Ok(Json(contracts::CloseEditorFlowDraftResponse {
        schema_id: contracts::CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_kind: contracts::DomainReceiptCommandKind::CloseEditorFlowDraft,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            expected_heads: vec![],
            prior_heads: vec![],
            resulting_heads: vec![],
            authoritative_revision_ids: vec![],
            proposal_revision_ids: vec![],
            authoritative_commit_ids: vec![],
            author_action_sequence,
            draft_artifact_refs: vec![input.draft_id.clone()],
            artifact_lifecycle_event_refs: event_id.into_iter().collect(),
            condition_refs: vec![],
            result,
            created_at: settlement.receipt_created_at,
        },
        effect,
    }))
}

pub(super) fn closed_event(
    scope: &ProjectScope,
    draft_id: &str,
    draft_revision_id: &str,
    payload_digest: &str,
    closed: &storyos_application::RefusedEditDraftClosure,
) -> Result<contracts::EditorFlowDraftClosed, ApiError> {
    let profile = if closed
        .command_digest
        .starts_with("sha256:storyos.command.expandRefusedEditDraftToProposal.jcs.v1:")
    {
        contracts::EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE
    } else if closed.close_reason == "superseded" {
        "storyos.command.applyAuthorEdit.jcs.v1"
    } else {
        contracts::CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE
    };
    let prefix = format!("sha256:{profile}:");
    let value_hex_lowercase = closed
        .command_digest
        .strip_prefix(&prefix)
        .filter(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or_else(challenge_store_unavailable)?
        .to_owned();
    Ok(contracts::EditorFlowDraftClosed {
        schema_id: contracts::EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID.to_owned(),
        event_kind: "editor_flow_draft_closed".to_owned(),
        event_id: closed.event_id.clone(),
        project_scope: contract_scope(scope),
        draft_id: draft_id.to_owned(),
        draft_revision_id: draft_revision_id.to_owned(),
        payload_digest: payload_digest.to_owned(),
        prior_closure: "open".to_owned(),
        closure: "closed".to_owned(),
        close_reason: closed.close_reason.clone(),
        source: contracts::RefusedEditDraftSource {
            command_id: closed.source.command_id.clone(),
            author_command_admission_id: closed.source.author_command_admission_id.clone(),
            receipt_id: closed.source.receipt_id.clone(),
            idempotency_key: closed.idempotency_key.clone(),
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: profile.to_owned(),
                value_hex_lowercase,
            },
        },
        author_action_sequence: closed.author_action_sequence.clone(),
        created_at: closed.created_at.clone(),
    })
}
