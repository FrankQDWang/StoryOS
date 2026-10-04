use storyos_application::{
    ChapterDeleted, DeleteChapterInput, DeleteChapterSettlement, ProjectCommandEnvelope,
    ProjectCommandError,
};
use storyos_core::{
    ChapterJoin, ChapterRemovalLifecycle, DeleteChapter as CoreDeleteChapter, DeleteChapterApplied,
    DeleteChapterConflict, DeleteChapterCurrent, DeleteChapterNoEffect, DeleteChapterRefusal,
    delete_chapter as classify_delete_chapter,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    AdmissionClass, Classified, CommandIsolation, CommandSpec, CurrentChapterChange, LockedProject,
    MissingAdmission, ProjectCommand, ResponseRecord, Structural, StructureIdentity,
    StructureWrite, WriterBase, settle_project_command, unavailable,
};

impl PostgresProjectReader {
    /// Settles one author-initiated Chapter removal as a Manuscript Structure Transition.
    pub async fn delete_chapter(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &DeleteChapterInput,
    ) -> Result<DeleteChapterSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The parent Volume of the target Chapter.
pub(crate) struct ParentVolume(String);

impl ProjectCommand for DeleteChapterInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "deleteChapter",
        isolation: CommandIsolation::Serializable,
        admission: AdmissionClass::ExplicitProjectCommand,
        missing_admission: MissingAdmission::InvalidChallenge,
        response: ResponseRecord::Project,
        activity_kind: "chapter_deleted",
    };
    type Profile = Structural;
    type Applied = DeleteChapterApplied;
    type Plan = ParentVolume;
    type Effect = ChapterDeleted;
    type NoEffect = DeleteChapterNoEffect;
    type Conflict = DeleteChapterConflict;
    type Refusal = DeleteChapterRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classified<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let target = client
            .query_opt(
                "SELECT chapter.parent_volume_id::text, removal.chapter_id IS NOT NULL
                   FROM storyos.manuscript_objects AS chapter
                   LEFT JOIN storyos.chapter_removal_decisions AS removal
                     ON (removal.owner_user_id, removal.project_id, removal.chapter_id) =
                        (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
                  WHERE chapter.owner_user_id = $1::text::uuid
                    AND chapter.project_id = $2::text::uuid
                    AND chapter.manuscript_object_id = $3::text::uuid
                    AND chapter.object_kind = 'chapter'
                  FOR UPDATE OF chapter",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.chapter_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let (chapter_join, chapter_lifecycle, volume_id) = match target {
            Some(target) => (
                ChapterJoin::ExactScope,
                if target.get::<_, bool>(1) {
                    ChapterRemovalLifecycle::Removed
                } else {
                    ChapterRemovalLifecycle::Active
                },
                target.get::<_, String>(0),
            ),
            None => (
                ChapterJoin::Invalid,
                ChapterRemovalLifecycle::Active,
                String::new(),
            ),
        };
        let ordered_active_chapter_ids = client
            .query(
                "SELECT chapter.manuscript_object_id::text
                   FROM storyos.manuscript_objects AS chapter
                   JOIN storyos.manuscript_objects AS volume
                     ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                        (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
                    AND volume.object_kind = 'volume'
                  WHERE chapter.owner_user_id = $1::text::uuid
                    AND chapter.project_id = $2::text::uuid
                    AND chapter.object_kind = 'chapter'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE removal.owner_user_id = chapter.owner_user_id
                         AND removal.project_id = chapter.project_id
                         AND removal.chapter_id = chapter.manuscript_object_id
                    )
                  ORDER BY volume.tree_order, chapter.tree_order
                  FOR UPDATE",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(unavailable)?
            .iter()
            .map(|chapter| chapter.get::<_, String>(0))
            .collect::<Vec<_>>();
        let classified = classify_delete_chapter(&CoreDeleteChapter {
            chapter_join,
            chapter_lifecycle,
            expected_tree_revision: self.expected_tree_revision,
            current_tree_revision: project.tree_revision,
            current_lifecycle: project.lifecycle,
            chapter_id: self.chapter_id.as_ref().to_owned(),
            current_chapter_id: project.current_chapter_id.clone(),
            ordered_active_chapter_ids,
        });
        Ok(classified.map_applied(|applied| (applied, ParentVolume(volume_id))))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        ParentVolume(volume_id): ParentVolume,
        applied: DeleteChapterApplied,
    ) -> Result<StructureWrite<ChapterDeleted>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.chapter_removal_decisions
                   (owner_user_id, project_id, chapter_removal_decision_id, receipt_id,
                    chapter_id, volume_id, tree_revision)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7::text::bigint)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &envelope.ids.receipt_id,
                    &self.chapter_id.as_ref(),
                    &volume_id,
                    &applied.tree_revision.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let (current_chapter, resulting_current) = match &applied.current {
            DeleteChapterCurrent::PreserveExisting => (
                CurrentChapterChange::Preserve,
                project.current_chapter_id.clone(),
            ),
            DeleteChapterCurrent::SelectSuccessor { chapter_id } => (
                CurrentChapterChange::Select(chapter_id.clone()),
                Some(chapter_id.clone()),
            ),
            DeleteChapterCurrent::Empty => (CurrentChapterChange::Clear, None),
        };
        Ok(StructureWrite {
            resulting_tree_revision: applied.tree_revision,
            identity: StructureIdentity::Chapter(self.chapter_id.as_ref().to_owned()),
            current_chapter,
            writer_base: WriterBase::RebindToCurrentChapter,
            activity: serde_json::json!({
                "chapter_id": self.chapter_id.as_ref(),
                "volume_id": volume_id,
                "current_chapter_id": resulting_current,
                "prior_current_chapter_id": project.current_chapter_id,
            }),
            effect: ChapterDeleted {
                volume_id,
                tree_revision: applied.tree_revision,
                current: applied.current,
            },
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ChapterDeleted, ReplayFault> {
        let tree_revision = replay.activity_u64("tree_revision")?;
        let volume_id = replay.activity_text("volume_id")?;
        let current = match (
            replay.activity_optional_text("prior_current_chapter_id"),
            replay.activity_optional_text("current_chapter_id"),
        ) {
            // The resulting Current alone cannot show that a different prior Current stayed.
            (Some(prior), _) if prior != self.chapter_id.as_ref() => {
                DeleteChapterCurrent::PreserveExisting
            }
            (_, None) => DeleteChapterCurrent::Empty,
            (_, Some(chapter_id)) => DeleteChapterCurrent::SelectSuccessor { chapter_id },
        };
        Ok(ChapterDeleted {
            volume_id,
            tree_revision,
            current,
        })
    }
}
