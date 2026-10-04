use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, UpdateVolumeInput, UpdateVolumeSettlement,
};
use storyos_core::{
    UpdateVolume as CoreUpdateVolume, UpdateVolumeApplied, UpdateVolumeConflict,
    UpdateVolumeNoEffect, UpdateVolumeRefusal, VolumeJoin, update_volume as classify_update_volume,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    Classification, CommandIsolation, CommandSpec, CurrentChapterChange, LockedProject,
    MissingAdmission, ProjectCommand, ProjectResponse, Structural, StructureIdentity,
    StructureWrite, WriterBase, settle_project_command, unavailable,
};
use crate::structural_authority_settlement::StructureTransitionSequences;

impl PostgresProjectReader {
    /// Settles one Update Volume (rename and reorder) as a Manuscript Structure Transition.
    pub async fn update_volume(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &UpdateVolumeInput,
    ) -> Result<UpdateVolumeSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The live Volume order that `classify` locks and `apply` reuses.
pub(crate) struct LiveVolumes {
    ordered_ids: Vec<String>,
    current_title: String,
    current_order: u64,
}

impl ProjectCommand for UpdateVolumeInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "updateVolume",
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        activity_kind: "volume_updated",
    };
    type Profile = Structural;
    type Response = ProjectResponse;
    type Applied = UpdateVolumeApplied;
    type Plan = LiveVolumes;
    type Effect = UpdateVolumeApplied;
    type NoEffect = UpdateVolumeNoEffect;
    type Conflict = UpdateVolumeConflict;
    type Refusal = UpdateVolumeRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let volumes = client
            .query(
                "SELECT manuscript_object_id::text, title
                   FROM storyos.manuscript_objects AS volume
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND object_kind = 'volume'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.volume_removal_decisions AS removal
                       WHERE removal.owner_user_id = volume.owner_user_id
                         AND removal.project_id = volume.project_id
                         AND removal.volume_id = volume.manuscript_object_id
                    )
                  ORDER BY tree_order
                  FOR UPDATE",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let ordered_ids = volumes
            .iter()
            .map(|volume| volume.get::<_, String>(0))
            .collect::<Vec<_>>();
        let (volume_join, current_title, current_order) = match ordered_ids
            .iter()
            .position(|volume_id| volume_id == self.volume_id.as_ref())
        {
            Some(index) => (
                VolumeJoin::ExactScope,
                volumes[index].get::<_, String>(1),
                index as u64 + 1,
            ),
            None => (VolumeJoin::Invalid, String::new(), 1),
        };
        let classified = classify_update_volume(&CoreUpdateVolume {
            volume_join,
            expected_tree_revision: self.expected_tree_revision,
            current_tree_revision: project.tree_revision,
            current_lifecycle: project.lifecycle,
            title: self.title.clone(),
            current_title: current_title.clone(),
            order: self.order,
            current_order,
            volume_count: ordered_ids.len() as u64,
        });
        Ok(Classification::project_command(classified.map_applied(
            |applied| {
                (
                    applied,
                    LiveVolumes {
                        ordered_ids,
                        current_title,
                        current_order,
                    },
                )
            },
        )))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &StructureTransitionSequences,
        live: LiveVolumes,
        applied: UpdateVolumeApplied,
    ) -> Result<StructureWrite<UpdateVolumeApplied>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let renamed = client
            .execute(
                "UPDATE storyos.manuscript_objects
                    SET title = $3
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND manuscript_object_id = $4::text::uuid AND object_kind = 'volume'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &applied.title,
                    &self.volume_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if renamed != 1 {
            return Err(unavailable("Volume row changed under FOR UPDATE"));
        }
        if live.current_order != applied.order {
            let mut ids = live.ordered_ids;
            let moved = ids.remove((live.current_order - 1) as usize);
            ids.insert((applied.order - 1) as usize, moved);
            crate::volume_storage_order::persist_volume_storage_order(client, scope, &ids)
                .await
                .map_err(ProjectCommandError::Unavailable)?;
        }
        Ok(StructureWrite {
            resulting_tree_revision: applied.tree_revision,
            identity: StructureIdentity::Volume(self.volume_id.as_ref().to_owned()),
            current_chapter: CurrentChapterChange::Preserve,
            writer_base: WriterBase::Keep,
            activity: serde_json::json!({
                "volume_id": self.volume_id.as_ref(),
                "title": applied.title,
                "order": applied.order.to_string(),
                "prior_title": live.current_title,
                "prior_order": live.current_order.to_string(),
            }),
            effect: applied,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<UpdateVolumeApplied, ReplayFault> {
        Ok(UpdateVolumeApplied {
            title: replay.activity_text("title")?,
            order: replay.activity_u64("order")?,
            tree_revision: replay.activity_u64("tree_revision")?,
        })
    }
}
