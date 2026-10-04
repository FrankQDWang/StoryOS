use storyos_application::{EditorSessionId, SetCurrentChapterInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    ProblemMapping, ProjectCommandRoute, ReceiptAuthority, RevisionMismatch, SchemaMismatch,
    SettledReceipt, admit,
};
use super::contract_reason::contract_reason;
use super::*;

const SET_CURRENT_CHAPTER: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Set Current Chapter",
    command_kind: "setCurrentChapter",
    method: contracts::SET_CURRENT_CHAPTER_METHOD,
    path: contracts::SET_CURRENT_CHAPTER_PATH,
    schema_id: contracts::SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID,
    digest_profile: contracts::SET_CURRENT_CHAPTER_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::SetCurrentChapter,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn set_current_chapter(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::SetCurrentChapterResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &SET_CURRENT_CHAPTER,
        |body: &contracts::SetCurrentChapterRequest| {
            let input = &body.set_current_chapter_input;
            valid_uuid(&input.chapter_id)?;
            valid_uuid(&input.expected_current_chapter_id)?;
            valid_uuid(&input.expected_target_revision_id)?;
            valid_uuid(&input.editor_session_id)?;
            Ok(SetCurrentChapterInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                chapter_id: input.chapter_id.clone(),
                expected_current_chapter_id: input.expected_current_chapter_id.clone(),
                expected_target_revision_id: input.expected_target_revision_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .set_current_chapter(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| SET_CURRENT_CHAPTER.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            authority = applied
                .authority
                .into_settled()
                .map(|authority| ReceiptAuthority {
                    authoritative_commit_ids: Vec::new(),
                    author_action_sequence: authority.author_action_sequence,
                });
            contracts::SetCurrentChapterEffect::AuthoritativeApplied {
                current_chapter_id: applied.effect.current_chapter_id,
                base_snapshot_id: applied.effect.base_snapshot_id,
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::SetCurrentChapterEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::SetCurrentChapterEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => contracts::SetCurrentChapterEffect::Refused {
            reason: contract_reason(&reason)?,
        },
    };
    let ack = admitted.acknowledgement(
        &SET_CURRENT_CHAPTER,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            heads: vec![admitted.input.expected_target_revision_id.clone()],
            project: settlement.response,
        },
    );
    Ok(Json(contracts::SetCurrentChapterResponse {
        schema_id: contracts::SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
