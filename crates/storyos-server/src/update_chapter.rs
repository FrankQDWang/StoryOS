use storyos_application::{ChapterId, UpdateChapterInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch, SettledReceipt, admit,
    positive, structure_title,
};
use super::contract_reason::contract_reason;
use super::*;

const UPDATE_CHAPTER: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Update Chapter",
    command_kind: "updateChapter",
    method: contracts::UPDATE_CHAPTER_METHOD,
    path: contracts::UPDATE_CHAPTER_PATH,
    schema_id: contracts::UPDATE_CHAPTER_REQUEST_SCHEMA_ID,
    digest_profile: contracts::UPDATE_CHAPTER_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::UpdateChapter,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn update_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, chapter_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::UpdateChapterResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&chapter_id],
        request,
        &UPDATE_CHAPTER,
        |body: &contracts::UpdateChapterRequest| {
            let input = &body.update_chapter_input;
            Ok(UpdateChapterInput {
                expected_tree_revision: positive(&input.expected_tree_revision)?,
                order: positive(&input.order)?,
                title: structure_title(&input.title)?,
                chapter_id: ChapterId::new(chapter_id.clone()),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .update_chapter(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| UPDATE_CHAPTER.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            authority = applied.authority.into_settled();
            contracts::UpdateChapterEffect::AuthoritativeApplied {
                chapter_id,
                title: applied.effect.title,
                tree_revision: applied.effect.tree_revision.to_string(),
                order: applied.effect.order.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::UpdateChapterEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::UpdateChapterEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => contracts::UpdateChapterEffect::Refused {
            reason: contract_reason(&reason)?,
        },
    };
    let ack = admitted.acknowledgement(
        &UPDATE_CHAPTER,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            project: settlement.response_project,
        },
    );
    Ok(Json(contracts::UpdateChapterResponse {
        schema_id: contracts::UPDATE_CHAPTER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
