use std::convert::Infallible;

use storyos_application::{
    CreateVolumeInput, CreateVolumePublicOrder, CreateVolumeSettlement, ProjectCommandEnvelope,
    ProjectCommandError, VolumeCreated,
};
use storyos_core::{
    CreateVolume as CoreCreateVolume, CreateVolumeApplied, CreateVolumeConflict,
    CreateVolumeRefusal, TransitionOutcome, create_volume as classify_create_volume,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    AppliedResult, Classification, CommandIsolation, CommandSpec, CurrentChapterChange,
    LockedProject, MissingAdmission, ProjectCommand, ProjectResponse, RateLimitedChallenge,
    ReplayEffect, Structural, StructureIdentity, StructureWrite, WriterBase,
    settle_project_command, unavailable,
};
use crate::structural_authority_settlement::StructureTransitionSequences;

impl PostgresProjectReader {
    /// Settles one Create Volume as a Manuscript Structure Transition.
    pub async fn create_volume(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &CreateVolumeInput,
    ) -> Result<CreateVolumeSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The Canonical Sibling Order of the new Volume among the live Volumes.
pub(crate) struct NewVolumeOrder(u64);

impl ProjectCommand for CreateVolumeInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "createVolume",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "volume_created",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Error = ProjectCommandError;
    type Profile = Structural;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = CreateVolumeApplied;
    type Plan = NewVolumeOrder;
    type Effect = VolumeCreated;
    type NoEffect = Infallible;
    type Conflict = CreateVolumeConflict;
    type Refusal = CreateVolumeRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let classified = classify_create_volume(&CoreCreateVolume {
            expected_tree_revision: self.expected_tree_revision,
            current_tree_revision: project.tree_revision,
            current_lifecycle: project.lifecycle,
            title: self.title.clone(),
        });
        Ok(Classification::project_command(match classified {
            TransitionOutcome::Applied(applied) => {
                let live_volumes = client
                    .query_one(
                        "SELECT count(*)::text
                           FROM storyos.manuscript_objects AS volume
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                            AND object_kind = 'volume'
                            AND NOT EXISTS (
                              SELECT 1 FROM storyos.volume_removal_decisions AS removal
                               WHERE removal.owner_user_id = volume.owner_user_id
                                 AND removal.project_id = volume.project_id
                                 AND removal.volume_id = volume.manuscript_object_id
                            )",
                        &[
                            &envelope.project_scope.owner_user_id.as_ref(),
                            &envelope.project_scope.project_id.as_ref(),
                        ],
                    )
                    .await
                    .map_err(unavailable)?
                    .get::<_, String>(0)
                    .parse::<u64>()
                    .map_err(unavailable)?;
                TransitionOutcome::Applied((applied, NewVolumeOrder(live_volumes + 1)))
            }
            TransitionOutcome::NoEffect(reason) => match reason {},
            TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
        }))
    }

    fn applied_receipt_payload(
        &self,
        _applied: &CreateVolumeApplied,
        order: &NewVolumeOrder,
    ) -> String {
        serde_json::json!({ "order": order.0.to_string() }).to_string()
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &StructureTransitionSequences,
        order: NewVolumeOrder,
        applied: CreateVolumeApplied,
    ) -> Result<StructureWrite<VolumeCreated>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let volume_id = Uuid::now_v7().to_string();
        client
            .execute(
                "INSERT INTO storyos.manuscript_objects
                   (owner_user_id, project_id, manuscript_object_id, object_kind, title, tree_order)
                 SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, 'volume', $4,
                        COALESCE(MAX(tree_order), 0) + 1
                   FROM storyos.manuscript_objects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND object_kind = 'volume'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &volume_id,
                    &self.title,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(StructureWrite {
            resulting_tree_revision: applied.tree_revision,
            identity: StructureIdentity::Volume(volume_id.clone()),
            current_chapter: CurrentChapterChange::Preserve,
            writer_base: WriterBase::Keep,
            activity: serde_json::json!({
                "volume_id": volume_id,
                "title": self.title,
                "order": order.0.to_string(),
            }),
            effect: VolumeCreated {
                volume_id,
                tree_revision: applied.tree_revision,
                order: CreateVolumePublicOrder::CanonicalSiblingOrder(order.0),
            },
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<VolumeCreated, ReplayFault> {
        let tree_revision = replay.activity_u64("tree_revision")?;
        let volume_id = replay.activity_uuid("volume_id")?;
        let order = match replay.receipt_historical_decimal("order")? {
            Some(0) => {
                return Err(ReplayFault::Unavailable(
                    "the stored sibling order is zero".into(),
                ));
            }
            Some(rank) => CreateVolumePublicOrder::CanonicalSiblingOrder(rank),
            None => CreateVolumePublicOrder::HistoricalCreateVolumeAck,
        };
        Ok(VolumeCreated {
            volume_id,
            tree_revision,
            order,
        })
    }
}
