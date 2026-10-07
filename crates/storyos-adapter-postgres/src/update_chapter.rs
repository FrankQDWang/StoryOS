use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, UpdateChapterInput, UpdateChapterSettlement,
};
use storyos_core::{
    ChapterJoin, UpdateChapter as CoreUpdateChapter, UpdateChapterApplied, UpdateChapterConflict,
    UpdateChapterNoEffect, UpdateChapterRefusal, update_chapter as classify_update_chapter,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    AppliedResult, Classification, CommandIsolation, CommandSpec, CurrentChapterChange,
    LockedProject, MissingAdmission, ProjectCommand, ProjectResponse, RateLimitedChallenge,
    ReplayEffect, Structural, StructureIdentity, StructureWrite, WriterBase,
    settle_project_command, unavailable,
};
use crate::structural_authority_settlement::StructureTransitionSequences;

pub(crate) mod sibling_order;

impl PostgresProjectReader {
    /// Settles one Update Chapter (rename and reorder) as a Manuscript Structure Transition.
    pub async fn update_chapter(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &UpdateChapterInput,
    ) -> Result<UpdateChapterSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The live sibling order of the target Chapter, locked by `classify`.
pub(crate) struct LiveSiblings {
    ordered_ids: Vec<String>,
    parent_volume_id: String,
    current_title: String,
    current_order: u64,
}

impl ProjectCommand for UpdateChapterInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "updateChapter",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "chapter_updated",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Profile = Structural;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = UpdateChapterApplied;
    type Plan = LiveSiblings;
    type Effect = UpdateChapterApplied;
    type NoEffect = UpdateChapterNoEffect;
    type Conflict = UpdateChapterConflict;
    type Refusal = UpdateChapterRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let chapters = client
            .query(
                "SELECT manuscript_object_id::text, title, parent_volume_id::text
                   FROM storyos.manuscript_objects AS chapter
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND object_kind = 'chapter'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE removal.owner_user_id = chapter.owner_user_id
                         AND removal.project_id = chapter.project_id
                         AND removal.chapter_id = chapter.manuscript_object_id
                    )
                  ORDER BY parent_volume_id, tree_order
                  FOR UPDATE",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let target = chapters
            .iter()
            .find(|chapter| chapter.get::<_, String>(0) == self.chapter_id.as_ref());
        let (chapter_join, siblings) = match target {
            Some(chapter) => {
                let parent_volume_id = chapter.get::<_, String>(2);
                let ordered_ids = chapters
                    .iter()
                    .filter(|row| row.get::<_, String>(2) == parent_volume_id)
                    .map(|row| row.get::<_, String>(0))
                    .collect::<Vec<_>>();
                let current_order = ordered_ids
                    .iter()
                    .position(|chapter_id| chapter_id == self.chapter_id.as_ref())
                    .map_or(1, |index| index as u64 + 1);
                (
                    ChapterJoin::ExactScope,
                    LiveSiblings {
                        ordered_ids,
                        parent_volume_id,
                        current_title: chapter.get(1),
                        current_order,
                    },
                )
            }
            None => (
                ChapterJoin::Invalid,
                LiveSiblings {
                    ordered_ids: Vec::new(),
                    parent_volume_id: String::new(),
                    current_title: String::new(),
                    current_order: 1,
                },
            ),
        };
        let classified = classify_update_chapter(&CoreUpdateChapter {
            chapter_join,
            expected_tree_revision: self.expected_tree_revision,
            current_tree_revision: project.tree_revision,
            current_lifecycle: project.lifecycle,
            title: self.title.clone(),
            current_title: siblings.current_title.clone(),
            order: self.order,
            current_order: siblings.current_order,
            chapter_count: siblings.ordered_ids.len() as u64,
        });
        Ok(Classification::project_command(
            classified.map_applied(|applied| (applied, siblings)),
        ))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &StructureTransitionSequences,
        siblings: LiveSiblings,
        applied: UpdateChapterApplied,
    ) -> Result<StructureWrite<UpdateChapterApplied>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let renamed = client
            .execute(
                "UPDATE storyos.manuscript_objects
                    SET title = $3
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND manuscript_object_id = $4::text::uuid AND object_kind = 'chapter'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &applied.title,
                    &self.chapter_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if renamed != 1 {
            return Err(unavailable("Chapter row changed under FOR UPDATE"));
        }
        if siblings.current_order != applied.order {
            let mut ids = siblings.ordered_ids;
            let moved = ids.remove((siblings.current_order - 1) as usize);
            ids.insert((applied.order - 1) as usize, moved);
            sibling_order::write_chapter_order(client, scope, &siblings.parent_volume_id, &ids)
                .await
                .map_err(unavailable)?;
        }
        Ok(StructureWrite {
            resulting_tree_revision: applied.tree_revision,
            identity: StructureIdentity::Chapter(self.chapter_id.as_ref().to_owned()),
            current_chapter: CurrentChapterChange::Preserve,
            writer_base: WriterBase::Keep,
            activity: serde_json::json!({
                "chapter_id": self.chapter_id.as_ref(),
                "title": applied.title,
                "order": applied.order.to_string(),
                "prior_title": siblings.current_title,
                "prior_order": siblings.current_order.to_string(),
            }),
            effect: applied,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<UpdateChapterApplied, ReplayFault> {
        let tree_revision = replay.activity_u64("tree_revision")?;
        Ok(UpdateChapterApplied {
            title: replay.activity_text("title")?,
            order: replay.activity_u64("order")?,
            tree_revision,
        })
    }
}
