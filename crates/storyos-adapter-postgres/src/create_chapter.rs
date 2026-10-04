use std::convert::Infallible;

use storyos_application::{
    ChapterCreated, CreateChapterInput, CreateChapterPublicOrder, CreateChapterSettlement,
    ProjectCommandEnvelope, ProjectCommandError,
};
use storyos_core::{
    CreateChapter as CoreCreateChapter, CreateChapterApplied, CreateChapterConflict,
    CreateChapterCurrent, CreateChapterOpen, CreateChapterPlacement, CreateChapterRefusal,
    VolumeJoin, create_chapter as classify_create_chapter,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::structure_command::{
    Classified, CommandIsolation, CommandSpec, CurrentChapterChange, LockedProject,
    StructureCommand, StructureIdentity, StructureWrite, WriterBase, settle_structure_command,
    unavailable,
};

impl PostgresProjectReader {
    /// Settles one Create Chapter as a Manuscript Structure Transition.
    pub async fn create_chapter(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &CreateChapterInput,
    ) -> Result<CreateChapterSettlement, ProjectCommandError> {
        settle_structure_command(self, envelope, input).await
    }
}

/// The live Chapter order of the target Volume, locked by `classify`.
pub(crate) struct LiveChapters(Vec<String>);

impl StructureCommand for CreateChapterInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "createChapter",
        isolation: CommandIsolation::Serializable,
        activity_kind: "chapter_created",
    };
    type Applied = CreateChapterApplied;
    type Plan = LiveChapters;
    type Effect = ChapterCreated;
    type NoEffect = Infallible;
    type Conflict = CreateChapterConflict;
    type Refusal = CreateChapterRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classified<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let volume_join = match client
            .query_opt(
                "SELECT manuscript_object_id
                   FROM storyos.manuscript_objects AS volume
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND manuscript_object_id = $3::text::uuid AND object_kind = 'volume'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.volume_removal_decisions AS removal
                       WHERE removal.owner_user_id = volume.owner_user_id
                         AND removal.project_id = volume.project_id
                         AND removal.volume_id = volume.manuscript_object_id
                    )",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.volume_id,
                ],
            )
            .await
            .map_err(unavailable)?
        {
            Some(_) => VolumeJoin::ExactScope,
            None => VolumeJoin::Invalid,
        };
        let ordered_chapter_ids = client
            .query(
                "SELECT manuscript_object_id::text FROM storyos.manuscript_objects AS chapter
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND object_kind = 'chapter' AND parent_volume_id = $3::text::uuid
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE removal.owner_user_id = chapter.owner_user_id
                         AND removal.project_id = chapter.project_id
                         AND removal.chapter_id = chapter.manuscript_object_id
                    )
                  ORDER BY tree_order FOR UPDATE",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.volume_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .iter()
            .map(|row| row.get::<_, String>(0))
            .collect::<Vec<_>>();
        let classified = classify_create_chapter(&CoreCreateChapter {
            volume_join,
            expected_tree_revision: self.expected_tree_revision,
            current_tree_revision: project.tree_revision,
            current_lifecycle: project.lifecycle,
            current_open: match project.current_chapter_id {
                None => CreateChapterOpen::Empty,
                Some(_) => CreateChapterOpen::CurrentChapter,
            },
            title: self.title.clone(),
            placement: self.placement.clone(),
            ordered_chapter_ids: ordered_chapter_ids.clone(),
        });
        Ok(classified.map_applied(|applied| (applied, LiveChapters(ordered_chapter_ids))))
    }

    fn applied_receipt_payload(
        &self,
        applied: &CreateChapterApplied,
        _plan: &LiveChapters,
    ) -> String {
        serde_json::json!({ "order": applied.order.to_string() }).to_string()
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        LiveChapters(mut ordered_chapter_ids): LiveChapters,
        applied: CreateChapterApplied,
    ) -> Result<StructureWrite<ChapterCreated>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let owner = scope.owner_user_id.as_ref();
        let project_id = scope.project_id.as_ref();
        let chapter_id = Uuid::now_v7().to_string();
        client
            .execute(
                "INSERT INTO storyos.manuscript_objects
                   (owner_user_id, project_id, manuscript_object_id, object_kind, title, tree_order,
                    parent_volume_id)
                 SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, 'chapter', $4,
                        COALESCE(MAX(tree_order), 0) + 1, $5::text::uuid
                   FROM storyos.manuscript_objects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND object_kind = 'chapter' AND parent_volume_id = $5::text::uuid",
                &[
                    &owner,
                    &project_id,
                    &chapter_id,
                    &self.title,
                    &self.volume_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if self.placement != CreateChapterPlacement::Append {
            ordered_chapter_ids.insert((applied.order - 1) as usize, chapter_id.clone());
            crate::update_chapter::sibling_order::write_chapter_order(
                client,
                scope,
                &self.volume_id,
                &ordered_chapter_ids,
            )
            .await
            .map_err(unavailable)?;
        }
        let payload_id = Uuid::now_v7().to_string();
        let revision_id = Uuid::now_v7().to_string();
        let empty: &[u8] = &[];
        client
            .execute(
                "INSERT INTO storyos.authoritative_payloads
                   (owner_user_id, project_id, payload_id, canonical_bytes)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4)",
                &[&owner, &project_id, &payload_id, &empty],
            )
            .await
            .map_err(unavailable)?;
        client
            .execute(
                "INSERT INTO storyos.authoritative_revisions
                   (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid)",
                &[&owner, &project_id, &chapter_id, &revision_id, &payload_id],
            )
            .await
            .map_err(unavailable)?;
        client
            .execute(
                "INSERT INTO storyos.authoritative_heads
                   (owner_user_id, project_id, manuscript_object_id, current_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
                &[&owner, &project_id, &chapter_id, &revision_id],
            )
            .await
            .map_err(unavailable)?;
        crate::manuscript_block::insert_paragraph_block(
            client,
            owner,
            project_id,
            &chapter_id,
            &revision_id,
            &Uuid::now_v7().to_string(),
        )
        .await
        .map_err(unavailable)?;
        let (current_chapter, resulting_current) = match applied.current {
            CreateChapterCurrent::SelectCreated => (
                CurrentChapterChange::Select(chapter_id.clone()),
                Some(chapter_id.clone()),
            ),
            CreateChapterCurrent::PreserveExisting => (
                CurrentChapterChange::Preserve,
                project.current_chapter_id.clone(),
            ),
        };
        Ok(StructureWrite {
            resulting_tree_revision: applied.tree_revision,
            identity: StructureIdentity::ChapterInitialRevision {
                chapter_id: chapter_id.clone(),
                revision_id,
            },
            current_chapter,
            writer_base: WriterBase::Keep,
            activity: serde_json::json!({
                "volume_id": self.volume_id,
                "chapter_id": chapter_id,
                "title": self.title,
                "order": applied.order.to_string(),
                "current_chapter_id": resulting_current,
            }),
            effect: ChapterCreated {
                chapter_id,
                tree_revision: applied.tree_revision,
                current: applied.current,
                order: CreateChapterPublicOrder::CanonicalSiblingOrder(applied.order),
            },
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ChapterCreated, ReplayFault> {
        let tree_revision = replay.activity_u64("tree_revision")?;
        let chapter_id = replay.activity_text("chapter_id")?;
        let resulting_current = replay.activity_text("current_chapter_id")?;
        let activity_order = replay.activity_u64("order")?;
        let order = match replay.receipt_text("order") {
            Some(order) => match order.parse::<u64>() {
                Ok(0) => return Err(ReplayFault::BindingConflict),
                Ok(rank) => CreateChapterPublicOrder::CanonicalSiblingOrder(rank),
                Err(error) => return Err(ReplayFault::Unavailable(Box::new(error))),
            },
            None => CreateChapterPublicOrder::HistoricalCreateChapterAck(activity_order),
        };
        Ok(ChapterCreated {
            tree_revision,
            current: if resulting_current == chapter_id {
                CreateChapterCurrent::SelectCreated
            } else {
                CreateChapterCurrent::PreserveExisting
            },
            chapter_id,
            order,
        })
    }
}
