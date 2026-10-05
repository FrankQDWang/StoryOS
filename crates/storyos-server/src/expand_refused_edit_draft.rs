use storyos_application::{
    DraftExpansionObservation, EditorSessionId, ExpandRefusedEditDraftToProposalInput,
    ProjectCommandError,
};
use storyos_core::{OpenInlineProposalAnchor, TransitionOutcome};

use super::command_admission::{
    AntiForgery, BodyValidation, HeaderCheck, ProblemMapping, ProjectCommandRoute,
    RevisionMismatch, SchemaMismatch, TargetValidation, admit_body, read_body,
};
use super::contract_reason::contract_reason;
use super::*;

const EXPAND_REFUSED_EDIT_DRAFT: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Draft expansion",
    command_kind: "expandRefusedEditDraftToProposal",
    method: "POST",
    path: contracts::EXPAND_REFUSED_EDIT_DRAFT_PATH,
    schema_id: contracts::EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
    digest_profile: contracts::EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Route(expansion_problem),
};

fn binding_conflict() -> ApiError {
    problem(
        StatusCode::CONFLICT,
        "draft_expansion_binding_conflict",
        "The Draft expansion binding conflicts.",
    )
}

fn store_unavailable() -> ApiError {
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "project_store_unavailable",
        "The Project store is unavailable.",
    )
}

fn expansion_problem(error: ProjectCommandError) -> ApiError {
    match error {
        ProjectCommandError::MissingProject => resource_unavailable(),
        ProjectCommandError::WriterIneligible
        | ProjectCommandError::BindingConflict
        | ProjectCommandError::HistoricalAcknowledgementUnavailable => binding_conflict(),
        ProjectCommandError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Draft expansion challenge is invalid.",
        ),
        ProjectCommandError::Unavailable(_) => store_unavailable(),
    }
}

pub(super) async fn expand_refused_edit_draft(
    State(state): State<Arc<ServerState>>,
    Path((project_id, draft_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ExpandRefusedEditDraftResponse>, ApiError> {
    // This route checks the session and command headers before the body parse, as on main.
    let read = read_body::<contracts::ExpandRefusedEditDraftRequest>(
        &state,
        &project_id,
        &[&draft_id],
        request,
        &EXPAND_REFUSED_EDIT_DRAFT,
        HeaderCheck::BeforeBodyParse,
    )
    .await?;
    let admitted = admit_body(
        &state,
        read,
        &EXPAND_REFUSED_EDIT_DRAFT,
        AntiForgery::Required,
        |body: &contracts::ExpandRefusedEditDraftRequest| {
            let input = &body.expand_refused_edit_draft_to_proposal_input;
            let writer_generation = input
                .writer_generation
                .parse::<u64>()
                .ok()
                .filter(|generation| generation.to_string() == input.writer_generation);
            let Some(writer_generation) = writer_generation else {
                return Err(invalid_request());
            };
            if input.draft_id != draft_id
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
            let ([target_ref], [expected_target_revision_id], [anchor]) = (
                input.target_refs.as_slice(),
                input.expected_target_revisions.as_slice(),
                input.anchors.as_slice(),
            ) else {
                return Err(binding_conflict());
            };
            if input.expected_source_draft_closure != "open"
                || input.proposal_kind != "inline_edit"
                || anchor.manuscript_block_id != *target_ref
                || anchor.base_authoritative_revision_id != *expected_target_revision_id
            {
                return Err(binding_conflict());
            }
            Ok(ExpandRefusedEditDraftToProposalInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                writer_generation,
                draft_id: draft_id.clone(),
                source_current_draft_revision_id: input.source_current_draft_revision_id.clone(),
                source_draft_payload_digest: input.source_draft_payload_digest.clone(),
                source_reopen_event_id: input.source_reopen_event_id.clone(),
                chapter_id: input.chapter_id.clone(),
                target_ref: target_ref.clone(),
                expected_target_revision_id: expected_target_revision_id.clone(),
                anchor: OpenInlineProposalAnchor {
                    manuscript_block_id: anchor.manuscript_block_id.clone(),
                    base_authoritative_revision_id: anchor.base_authoritative_revision_id.clone(),
                    manuscript_schema_version: anchor.manuscript_schema_version,
                    coordinate_profile: anchor.coordinate_profile.clone(),
                    from: anchor.from,
                    to: anchor.to,
                    boundary_profile: anchor.boundary_profile.clone(),
                    base_slice_digest: anchor.base_slice_digest.clone(),
                },
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .expand_refused_edit_draft(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| EXPAND_REFUSED_EDIT_DRAFT.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let envelope = &admitted.envelope;
    let input = &admitted.input;
    let observed =
        |observation: Option<DraftExpansionObservation>| observation.ok_or_else(store_unavailable);
    let (result, author_action_sequence, created, observation, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            let author_action_sequence = Some(applied.author_action_sequence.to_string());
            let expanded = applied.effect;
            let draft = &expanded.observation.draft;
            let event = super::close_editor_flow_draft::closed_event(
                &envelope.project_scope,
                &input.draft_id,
                &draft.draft_revision_id,
                &draft.payload_digest,
                &storyos_application::RefusedEditDraftClosure {
                    event_id: expanded.event_id.clone(),
                    source: settlement.ids.clone(),
                    command_digest: envelope.challenge_binding.canonical_command_digest.clone(),
                    idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
                    author_action_sequence: author_action_sequence.clone(),
                    close_reason: "superseded".to_owned(),
                    created_at: settlement.receipt_created_at.clone(),
                },
            )?;
            (
                contracts::DomainReceiptResult::ProposalCreatedFromDraft,
                author_action_sequence,
                Some((expanded.proposal_revision_id.clone(), expanded.event_id)),
                expanded.observation,
                contracts::ExpandRefusedEditDraftEffect::ProposalCreatedFromDraft {
                    proposal_id: expanded.proposal_id,
                    proposal_revision_id: expanded.proposal_revision_id,
                    event: Box::new(event),
                },
            )
        }
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(_) => {
            let observation = observed(settlement.zero_authority_effect)?;
            let draft = observation.draft.clone();
            (
                contracts::DomainReceiptResult::Conflicted,
                None,
                None,
                observation,
                contracts::ExpandRefusedEditDraftEffect::Conflicted {
                    current_revision_id: draft.draft_revision_id,
                    current_digest: draft.payload_digest,
                    current_closure: draft.observed_closure,
                },
            )
        }
        TransitionOutcome::Refused(reason) => {
            let observation = observed(settlement.zero_authority_effect)?;
            let current_closure = observation.draft.observed_closure.clone();
            (
                contracts::DomainReceiptResult::Refused,
                None,
                None,
                observation,
                contracts::ExpandRefusedEditDraftEffect::Refused {
                    reason: contract_reason(&reason)?,
                    current_closure,
                },
            )
        }
    };
    let (proposal_revision_ids, artifact_lifecycle_event_refs) = created
        .map(|(revision_id, event_id)| (vec![revision_id], vec![event_id]))
        .unwrap_or_default();
    let heads = observation
        .current_target_revision_id
        .into_iter()
        .collect::<Vec<_>>();
    let project_scope = contract_scope(&envelope.project_scope);
    Ok(Json(contracts::ExpandRefusedEditDraftResponse {
        schema_id: contracts::EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_kind: contracts::DomainReceiptCommandKind::ExpandRefusedEditDraftToProposal,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            expected_heads: vec![input.expected_target_revision_id.clone()],
            prior_heads: heads.clone(),
            resulting_heads: heads,
            authoritative_revision_ids: vec![],
            proposal_revision_ids,
            authoritative_commit_ids: vec![],
            author_action_sequence,
            draft_artifact_refs: vec![input.draft_id.clone()],
            artifact_lifecycle_event_refs,
            condition_refs: vec![],
            result,
            created_at: settlement.receipt_created_at,
        },
        effect,
    }))
}
