use storyos_application::{ChapterId, DeleteChapterInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit, controlled_project, positive,
};
use super::contract_reason::contract_reason;
use super::*;

const DELETE_CHAPTER: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Delete Chapter",
    command_kind: "deleteChapter",
    method: contracts::DELETE_CHAPTER_METHOD,
    path: contracts::DELETE_CHAPTER_PATH,
    schema_id: contracts::DELETE_CHAPTER_REQUEST_SCHEMA_ID,
    digest_profile: contracts::DELETE_CHAPTER_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn delete_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, chapter_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::DeleteChapterResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&chapter_id],
        request,
        &DELETE_CHAPTER,
        |body: &contracts::DeleteChapterRequest| {
            Ok(DeleteChapterInput {
                expected_tree_revision: positive(
                    &body.delete_chapter_input.expected_tree_revision,
                )?,
                chapter_id: ChapterId::new(chapter_id.clone()),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .delete_chapter(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| DELETE_CHAPTER.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            authority = applied.authority.into_settled().map(Into::into);
            contracts::DeleteChapterEffect::AuthoritativeApplied {
                chapter_id,
                volume_id: applied.effect.volume_id,
                tree_revision: applied.effect.tree_revision.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::DeleteChapterEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::DeleteChapterEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => contracts::DeleteChapterEffect::Refused {
            reason: contract_reason(&reason)?,
        },
    };
    let ack = admitted.acknowledgement(
        &DELETE_CHAPTER,
        contracts::DomainReceiptCommandKind::DeleteChapter,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::DeleteChapterResponse {
        schema_id: contracts::DELETE_CHAPTER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
