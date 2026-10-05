use storyos_application::{EditorSessionId, ProjectCommandError, TakeOverProjectWriterInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit,
};
use super::*;

const TAKE_OVER_PROJECT_WRITER: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Take Over Project Writer",
    command_kind: "takeOverProjectWriter",
    method: contracts::TAKE_OVER_PROJECT_WRITER_METHOD,
    path: contracts::TAKE_OVER_PROJECT_WRITER_PATH,
    schema_id: contracts::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
    digest_profile: "storyos.command.takeOverProjectWriter.jcs.v1",
    revision_mismatch: RevisionMismatch::AuthenticationRequired,
    body_validation: BodyValidation::BeforeRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
    schema_mismatch: SchemaMismatch::CommandTargetRefused,
    problem_mapping: ProblemMapping::Route(takeover_problem),
};

pub(super) async fn take_over_project_writer(
    State(state): State<Arc<ServerState>>,
    Path((project_id, editor_session_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::TakeOverProjectWriterResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&editor_session_id],
        request,
        &TAKE_OVER_PROJECT_WRITER,
        |body: &contracts::TakeOverProjectWriterRequest| {
            valid_uuid(&body.correlation_id)?;
            valid_uuid(&body.editor_session_id)?;
            let observed_writer_generation = body
                .observed_writer_generation
                .parse::<u64>()
                .ok()
                .filter(|generation| *generation >= 1)
                .ok_or_else(invalid_request)?;
            if body.editor_session_id != editor_session_id {
                return Err(invalid_request());
            }
            Ok(TakeOverProjectWriterInput {
                editor_session_id: EditorSessionId::new(&body.editor_session_id),
                observed_writer_generation,
                editor_contract_revision: body.editor_contract_revision.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .take_over_project_writer(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| TAKE_OVER_PROJECT_WRITER.problem(error))?;
    let (
        TransitionOutcome::NoEffect(
            storyos_core::TakeOverProjectWriterNoEffect::WriterTakeoverApplied,
        ),
        Some(takeover),
    ) = (settlement.outcome, settlement.zero_authority_effect)
    else {
        return Err(takeover_problem(ProjectCommandError::BindingConflict));
    };
    let heads = vec![takeover.resulting_head.clone()];
    let ack = admitted.acknowledgement(
        &TAKE_OVER_PROJECT_WRITER,
        contracts::DomainReceiptCommandKind::TakeOverProjectWriter,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result: storyos_core::ReceiptResult::NoEffect,
            authority: None,
            heads: heads.clone(),
        },
    );
    Ok(Json(contracts::TakeOverProjectWriterResponse {
        schema_id: contracts::TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        result: contracts::TakeOverProjectWriterResult::TakeoverApplied {
            prior_editor_session_id: takeover.prior_editor_session_id,
            prior_writer_generation: takeover.prior_writer_generation.to_string(),
            resulting_editor_session_id: takeover.resulting_editor_session_id,
            resulting_writer_generation: takeover.resulting_writer_generation.to_string(),
            resulting_snapshot_id: takeover.resulting_snapshot_id,
            resulting_snapshot_activity_position: takeover
                .resulting_snapshot_activity_position
                .to_string(),
            resulting_heads: heads,
        },
    }))
}

fn takeover_problem(error: ProjectCommandError) -> ApiError {
    match error {
        ProjectCommandError::BindingConflict
        | ProjectCommandError::HistoricalAcknowledgementUnavailable
        | ProjectCommandError::MissingProject => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The writer takeover binding conflicts.",
        ),
        ProjectCommandError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The command challenge is invalid or expired.",
        ),
        ProjectCommandError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "author_edit_store_unavailable",
            "The writer takeover store is unavailable.",
        ),
    }
}
