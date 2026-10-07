use storyos_application::{UpdateVolumeInput, VolumeId};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit, controlled_project, positive, structure_title,
};
use super::contract_reason::contract_reason;
use super::*;

const UPDATE_VOLUME: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Update Volume",
    command_kind: "updateVolume",
    method: contracts::UPDATE_VOLUME_METHOD,
    path: contracts::UPDATE_VOLUME_PATH,
    schema_id: contracts::UPDATE_VOLUME_REQUEST_SCHEMA_ID,
    digest_profile: contracts::UPDATE_VOLUME_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn update_volume(
    State(state): State<Arc<ServerState>>,
    Path((project_id, volume_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::UpdateVolumeResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&volume_id],
        request,
        &UPDATE_VOLUME,
        |body: &contracts::UpdateVolumeRequest| {
            let input = &body.update_volume_input;
            Ok(UpdateVolumeInput {
                expected_tree_revision: positive(&input.expected_tree_revision)?,
                order: positive(&input.order)?,
                title: structure_title(&input.title)?,
                volume_id: VolumeId::new(volume_id.clone()),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .update_volume(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| UPDATE_VOLUME.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let mut authority = None;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            authority = applied.authority.into_settled().map(Into::into);
            contracts::UpdateVolumeEffect::AuthoritativeApplied {
                volume_id,
                title: applied.effect.title,
                tree_revision: applied.effect.tree_revision.to_string(),
                order: applied.effect.order.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::UpdateVolumeEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::UpdateVolumeEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => contracts::UpdateVolumeEffect::Refused {
            reason: contract_reason(&reason)?,
        },
    };
    let ack = admitted.acknowledgement(
        &UPDATE_VOLUME,
        contracts::DomainReceiptCommandKind::UpdateVolume,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::UpdateVolumeResponse {
        schema_id: contracts::UPDATE_VOLUME_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
