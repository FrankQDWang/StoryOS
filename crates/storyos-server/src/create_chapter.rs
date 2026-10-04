use storyos_application::{CreateChapterInput, CreateChapterPublicOrder};
use storyos_core::TransitionOutcome;

use super::contract_reason::contract_reason;
use super::structure_admission::{
    SettledReceipt, StructureRoute, admit, positive, structure_title,
};
use super::*;

const CREATE_CHAPTER: StructureRoute = StructureRoute {
    display_name: "Create Chapter",
    command_kind: "createChapter",
    method: contracts::CREATE_CHAPTER_METHOD,
    path: contracts::CREATE_CHAPTER_PATH,
    schema_id: contracts::CREATE_CHAPTER_REQUEST_SCHEMA_ID,
    digest_profile: contracts::CREATE_CHAPTER_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::CreateChapter,
};

pub(super) async fn create_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, volume_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CreateChapterResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&volume_id],
        request,
        &CREATE_CHAPTER,
        |body: &contracts::CreateChapterRequest| {
            let input = &body.create_chapter_input;
            let expected_tree_revision = positive(&input.expected_tree_revision)?;
            let title = structure_title(&input.title)?;
            let placement = match &input.placement {
                None => storyos_core::CreateChapterPlacement::Append,
                Some(contracts::CreateChapterPlacement::Before { chapter_id }) => {
                    valid_uuid(chapter_id)?;
                    storyos_core::CreateChapterPlacement::Before {
                        chapter_id: chapter_id.clone(),
                    }
                }
                Some(contracts::CreateChapterPlacement::After { chapter_id }) => {
                    valid_uuid(chapter_id)?;
                    storyos_core::CreateChapterPlacement::After {
                        chapter_id: chapter_id.clone(),
                    }
                }
            };
            Ok(CreateChapterInput {
                volume_id: volume_id.clone(),
                title,
                placement,
                expected_tree_revision,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .create_chapter(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| CREATE_CHAPTER.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            let order = match applied.effect.order {
                CreateChapterPublicOrder::CanonicalSiblingOrder(rank) => {
                    authority = applied.authority.into_settled();
                    rank
                }
                CreateChapterPublicOrder::HistoricalCreateChapterAck(storage_key) => storage_key,
            };
            let current_chapter_id = settlement
                .response_project
                .current_chapter_id
                .as_ref()
                .ok_or_else(resource_unavailable)?
                .as_ref()
                .to_owned();
            contracts::CreateChapterEffect::AuthoritativeApplied {
                volume_id,
                chapter_id: applied.effect.chapter_id,
                title: admitted.input.title.clone(),
                tree_revision: applied.effect.tree_revision.to_string(),
                order: order.to_string(),
                current_chapter_id,
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => contracts::CreateChapterEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => contracts::CreateChapterEffect::Refused {
            reason: contract_reason(&reason)?,
        },
    };
    let ack = admitted.acknowledgement(
        &CREATE_CHAPTER,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            project: settlement.response_project,
        },
    );
    Ok(Json(contracts::CreateChapterResponse {
        schema_id: contracts::CREATE_CHAPTER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
