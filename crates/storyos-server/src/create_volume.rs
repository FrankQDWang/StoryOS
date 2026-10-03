use storyos_application::{CreateVolumeInput, CreateVolumePublicOrder};
use storyos_core::TransitionOutcome;

use super::contract_reason::contract_reason;
use super::structure_admission::{
    SettledReceipt, StructureRoute, admit, positive, structure_title,
};
use super::*;

const CREATE_VOLUME: StructureRoute = StructureRoute {
    display_name: "Create Volume",
    command_kind: "createVolume",
    method: contracts::CREATE_VOLUME_METHOD,
    path: contracts::CREATE_VOLUME_PATH,
    schema_id: contracts::CREATE_VOLUME_REQUEST_SCHEMA_ID,
    digest_profile: contracts::CREATE_VOLUME_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::CreateVolume,
};

pub(super) async fn create_volume(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::CreateVolumeResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &CREATE_VOLUME,
        |body: &contracts::CreateVolumeRequest| {
            let input = &body.create_volume_input;
            Ok(CreateVolumeInput {
                expected_tree_revision: positive(&input.expected_tree_revision)?,
                title: structure_title(&input.title)?,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .create_volume(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| CREATE_VOLUME.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let (result, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            let order = match applied.effect.order {
                CreateVolumePublicOrder::CanonicalSiblingOrder(rank) => {
                    authority = applied.authority.into_settled();
                    rank.to_string()
                }
                CreateVolumePublicOrder::HistoricalCreateVolumeAck => "1".to_owned(),
            };
            (
                contracts::DomainReceiptResult::AuthoritativeApplied,
                contracts::CreateVolumeEffect::AuthoritativeApplied {
                    volume_id: applied.effect.volume_id,
                    title: admitted.input.title.clone(),
                    tree_revision: applied.effect.tree_revision.to_string(),
                    order,
                    project_activity_position: applied.project_activity_position.to_string(),
                },
            )
        }
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::CreateVolumeEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::DomainReceiptResult::Refused,
            contracts::CreateVolumeEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let ack = admitted.acknowledgement(
        &CREATE_VOLUME,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            project: settlement.response_project,
        },
    );
    Ok(Json(contracts::CreateVolumeResponse {
        schema_id: contracts::CREATE_VOLUME_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
