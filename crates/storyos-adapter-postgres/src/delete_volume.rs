use storyos_application::{
    DeleteVolumeInput, DeleteVolumeSettlement, ProjectCommandEnvelope, ProjectCommandError,
    VolumeDeleted,
};
use storyos_core::{
    DeleteVolume as CoreDeleteVolume, DeleteVolumeApplied, DeleteVolumeConflict,
    DeleteVolumeNoEffect, DeleteVolumeRefusal, VolumeChildPolicy, VolumeJoin,
    VolumeRemovalLifecycle, delete_volume as classify_delete_volume,
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
    /// Settles one author-initiated Volume removal as a Manuscript Structure Transition.
    pub async fn delete_volume(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &DeleteVolumeInput,
    ) -> Result<DeleteVolumeSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

impl ProjectCommand for DeleteVolumeInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "deleteVolume",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "volume_deleted",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Profile = Structural;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = DeleteVolumeApplied;
    type Plan = ();
    type Effect = VolumeDeleted;
    type NoEffect = DeleteVolumeNoEffect;
    type Conflict = DeleteVolumeConflict;
    type Refusal = DeleteVolumeRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let target = client
            .query_opt(
                "SELECT removal.volume_id IS NOT NULL
                   FROM storyos.manuscript_objects AS volume
                   LEFT JOIN storyos.volume_removal_decisions AS removal
                     ON (removal.owner_user_id, removal.project_id, removal.volume_id) =
                        (volume.owner_user_id, volume.project_id, volume.manuscript_object_id)
                  WHERE volume.owner_user_id = $1::text::uuid AND volume.project_id = $2::text::uuid
                    AND volume.manuscript_object_id = $3::text::uuid
                    AND volume.object_kind = 'volume'
                  FOR UPDATE OF volume",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.volume_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let (volume_join, volume_lifecycle) = match target {
            Some(target) if target.get::<_, bool>(0) => {
                (VolumeJoin::ExactScope, VolumeRemovalLifecycle::Removed)
            }
            Some(_) => (VolumeJoin::ExactScope, VolumeRemovalLifecycle::Active),
            None => (VolumeJoin::Invalid, VolumeRemovalLifecycle::Active),
        };
        let child_chapters = match volume_join {
            VolumeJoin::ExactScope => {
                let active_chapters = client
                    .query_one(
                        "SELECT count(*)::text
                           FROM storyos.manuscript_objects AS chapter
                          WHERE chapter.owner_user_id = $1::text::uuid
                            AND chapter.project_id = $2::text::uuid
                            AND chapter.parent_volume_id = $3::text::uuid
                            AND chapter.object_kind = 'chapter'
                            AND NOT EXISTS (
                              SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                               WHERE removal.owner_user_id = chapter.owner_user_id
                                 AND removal.project_id = chapter.project_id
                                 AND removal.chapter_id = chapter.manuscript_object_id
                            )",
                        &[
                            &scope.owner_user_id.as_ref(),
                            &scope.project_id.as_ref(),
                            &self.volume_id.as_ref(),
                        ],
                    )
                    .await
                    .map_err(unavailable)?
                    .get::<_, String>(0);
                if active_chapters == "0" {
                    VolumeChildPolicy::Empty
                } else {
                    VolumeChildPolicy::Nonempty
                }
            }
            VolumeJoin::Invalid => VolumeChildPolicy::Empty,
        };
        let classified = classify_delete_volume(&CoreDeleteVolume {
            volume_join,
            volume_lifecycle,
            child_chapters,
            expected_tree_revision: self.expected_tree_revision,
            current_tree_revision: project.tree_revision,
            current_lifecycle: project.lifecycle,
        });
        Ok(Classification::project_command(
            classified.map_applied(|applied| (applied, ())),
        ))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &StructureTransitionSequences,
        _plan: (),
        applied: DeleteVolumeApplied,
    ) -> Result<StructureWrite<VolumeDeleted>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.volume_removal_decisions
                   (owner_user_id, project_id, volume_removal_decision_id, receipt_id,
                    volume_id, tree_revision)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::bigint)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &envelope.ids.receipt_id,
                    &self.volume_id.as_ref(),
                    &applied.tree_revision.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(StructureWrite {
            resulting_tree_revision: applied.tree_revision,
            identity: StructureIdentity::Volume(self.volume_id.as_ref().to_owned()),
            current_chapter: CurrentChapterChange::Preserve,
            writer_base: WriterBase::RebindToCurrentChapter,
            activity: serde_json::json!({ "volume_id": self.volume_id.as_ref() }),
            effect: VolumeDeleted {
                volume_id: self.volume_id.as_ref().to_owned(),
                tree_revision: applied.tree_revision,
            },
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<VolumeDeleted, ReplayFault> {
        let tree_revision = replay.activity_u64("tree_revision")?;
        Ok(VolumeDeleted {
            volume_id: replay.activity_uuid("volume_id")?,
            tree_revision,
        })
    }
}
