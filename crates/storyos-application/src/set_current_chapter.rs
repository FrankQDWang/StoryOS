//! The input and settlement of the Set Current Chapter command.

use storyos_core::{
    SetCurrentChapterConflict, SetCurrentChapterNoEffect, SetCurrentChapterRefusal,
};

use crate::{ChapterSelectionApplied, EditorSessionId, ProjectCommandSettlement};

/// One Current Chapter change that the writer Editor Session requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetCurrentChapterInput {
    pub editor_session_id: EditorSessionId,
    pub chapter_id: String,
    pub expected_current_chapter_id: String,
    pub expected_target_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentChapterSelected {
    pub current_chapter_id: String,
    pub base_snapshot_id: String,
}

pub type SetCurrentChapterSettlement = ProjectCommandSettlement<
    ChapterSelectionApplied<CurrentChapterSelected>,
    SetCurrentChapterNoEffect,
    SetCurrentChapterConflict,
    SetCurrentChapterRefusal,
>;
