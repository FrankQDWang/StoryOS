use storyos_application::{DeleteVolumeInput, VolumeId};
use storyos_core::TransitionOutcome;

use super::contract_reason::contract_reason;
use super::structure_admission::{SettledReceipt, StructureRoute, admit, positive};
use super::*;

const DELETE_VOLUME: StructureRoute = StructureRoute {
    display_name: "Delete Volume",
    command_kind: "deleteVolume",
    method: contracts::DELETE_VOLUME_METHOD,
    path: contracts::DELETE_VOLUME_PATH,
    schema_id: contracts::DELETE_VOLUME_REQUEST_SCHEMA_ID,
    digest_profile: contracts::DELETE_VOLUME_DIGEST_PROFILE,
    receipt_kind: contracts::DomainReceiptCommandKind::DeleteVolume,
};

pub(super) async fn delete_volume(
    State(state): State<Arc<ServerState>>,
    Path((project_id, volume_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::DeleteVolumeResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&volume_id],
        request,
        &DELETE_VOLUME,
        |body: &contracts::DeleteVolumeRequest| {
            Ok(DeleteVolumeInput {
                expected_tree_revision: positive(&body.delete_volume_input.expected_tree_revision)?,
                volume_id: VolumeId::new(volume_id.clone()),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .delete_volume(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| DELETE_VOLUME.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let (result, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            authority = applied.authority.into_settled();
            (
                contracts::DomainReceiptResult::AuthoritativeApplied,
                contracts::DeleteVolumeEffect::AuthoritativeApplied {
                    volume_id: applied.effect.volume_id,
                    tree_revision: applied.effect.tree_revision.to_string(),
                    project_activity_position: applied.project_activity_position.to_string(),
                },
            )
        }
        TransitionOutcome::NoEffect(reason) => (
            contracts::DomainReceiptResult::NoEffect,
            contracts::DeleteVolumeEffect::NoEffect {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Conflicted(reason) => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::DeleteVolumeEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::DomainReceiptResult::Refused,
            contracts::DeleteVolumeEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let ack = admitted.acknowledgement(
        &DELETE_VOLUME,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            project: settlement.response_project,
        },
    );
    Ok(Json(contracts::DeleteVolumeResponse {
        schema_id: contracts::DELETE_VOLUME_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: ack.project,
        effect,
    }))
}
