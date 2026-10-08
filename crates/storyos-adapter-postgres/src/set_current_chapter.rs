use storyos_application::{
    CurrentChapterSelected, ProjectCommandEnvelope, ProjectCommandError, SetCurrentChapterInput,
    SetCurrentChapterSettlement,
};
use storyos_core::{
    ChapterJoin, SetCurrentChapter as CoreSetCurrentChapter, SetCurrentChapterConflict,
    SetCurrentChapterNoEffect, SetCurrentChapterRefusal,
    set_current_chapter as classify_set_current_chapter,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    Admission, AppliedVariant, ChapterSelection, ChapterSelectionWrite, Classification,
    CommandIsolation, CommandSpec, EditorAdmission, EditorWriter, LockedProject, MissingAdmission,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect, ZeroReceipt,
    settle_project_command, unavailable,
};
use crate::structural_authority_settlement::CurrentChapterSequences;
use crate::undo_compensation::ForwardCommand;

mod compensation;
pub(crate) use compensation::{CurrentChapterCompensation, ObservedCurrentChapterFrontier};

impl PostgresProjectReader {
    /// Settles one Current Chapter change of the writer Editor Session.
    pub async fn set_current_chapter(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &SetCurrentChapterInput,
    ) -> Result<SetCurrentChapterSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The current Authoritative Revision of the selected Chapter, as the Receipt heads record it.
pub(crate) struct ChapterHead(String);

impl ProjectCommand for SetCurrentChapterInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "setCurrentChapter",
        applied: &[AppliedVariant::Forward(ForwardCommand::SetCurrentChapter)],
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "current_chapter_set",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Error = ProjectCommandError;
    type Profile = ChapterSelection;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = String;
    type Plan = ChapterHead;
    type Effect = CurrentChapterSelected;
    type NoEffect = SetCurrentChapterNoEffect;
    type Conflict = SetCurrentChapterConflict;
    type Refusal = SetCurrentChapterRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let row = client
            .query_one(
                "SELECT chapter.manuscript_object_id IS NOT NULL,
                        head.current_revision_id::text,
                        expected.revision_id IS NOT NULL
                   FROM (SELECT 1) AS one
              LEFT JOIN storyos.manuscript_objects AS chapter
                     ON chapter.owner_user_id = $1::text::uuid
                    AND chapter.project_id = $2::text::uuid
                    AND chapter.object_kind = 'chapter'
                    AND chapter.manuscript_object_id = $3::text::uuid
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE removal.owner_user_id = chapter.owner_user_id
                         AND removal.project_id = chapter.project_id
                         AND removal.chapter_id = chapter.manuscript_object_id
                    )
              LEFT JOIN storyos.authoritative_heads AS head
                     ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                        (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
              LEFT JOIN storyos.authoritative_revisions AS expected
                     ON (expected.owner_user_id, expected.project_id,
                         expected.manuscript_object_id, expected.revision_id) =
                        (chapter.owner_user_id, chapter.project_id,
                         chapter.manuscript_object_id, $4::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.chapter_id,
                    &self.expected_target_revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        let chapter_join = if row.get::<_, bool>(/*idx*/ 0) {
            ChapterJoin::ExactScope
        } else {
            ChapterJoin::Invalid
        };
        let current_target_revision_id =
            row.get::<_, Option<String>>(/*idx*/ 1).unwrap_or_default();
        let expected_revision_exists = row.get::<_, bool>(/*idx*/ 2);
        let outcome = classify_set_current_chapter(&CoreSetCurrentChapter {
            chapter_join: chapter_join.clone(),
            current_lifecycle: project.lifecycle,
            current_chapter_id: project.current_chapter_id.clone(),
            expected_current_chapter_id: self.expected_current_chapter_id.clone(),
            target_chapter_id: self.chapter_id.clone(),
            expected_target_revision_id: self.expected_target_revision_id.clone(),
            current_target_revision_id: current_target_revision_id.clone(),
        });
        let resulting_head = if current_target_revision_id.is_empty() {
            self.expected_target_revision_id.clone()
        } else {
            current_target_revision_id.clone()
        };
        let (chapter_object_id, expected_authoritative_revision_id) =
            match (chapter_join, expected_revision_exists) {
                (ChapterJoin::Invalid, _) => (None, None),
                (ChapterJoin::ExactScope, true) => (
                    Some(self.chapter_id.clone()),
                    Some(self.expected_target_revision_id.clone()),
                ),
                (ChapterJoin::ExactScope, false) => (
                    Some(self.chapter_id.clone()),
                    Some(current_target_revision_id),
                ),
            };
        Ok(Classification {
            outcome: outcome
                .map_applied(|chapter_id| (chapter_id, ChapterHead(resulting_head.clone()))),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id,
                expected_authoritative_revision_id,
                target_refs: Vec::new(),
                writer: EditorWriter::Current,
            }),
            heads: ReceiptHeads {
                expected: vec![self.expected_target_revision_id.clone()],
                prior: vec![resulting_head.clone()],
                resulting: vec![resulting_head],
            },
            zero_receipt: ZeroReceipt::Reason,
        })
    }

    async fn apply(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        sequences: &CurrentChapterSequences,
        ChapterHead(chapter_revision_id): ChapterHead,
        chapter_id: String,
    ) -> Result<ChapterSelectionWrite<CurrentChapterSelected>, ProjectCommandError> {
        Ok(ChapterSelectionWrite {
            effect: CurrentChapterSelected {
                current_chapter_id: chapter_id.clone(),
                base_snapshot_id: sequences.snapshot_id.clone(),
            },
            editor_session_id: self.editor_session_id.as_ref().to_owned(),
            prior_chapter_id: self.expected_current_chapter_id.clone(),
            chapter_id,
            chapter_revision_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<CurrentChapterSelected, ReplayFault> {
        Ok(CurrentChapterSelected {
            current_chapter_id: replay.activity_uuid("current_chapter_id")?,
            base_snapshot_id: replay.activity_uuid("base_snapshot_id")?,
        })
    }
}
