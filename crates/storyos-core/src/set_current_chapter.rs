//! Pure Core classification for Set Current Chapter.

use super::{ChapterJoin, ProjectLifecycle, TransitionOutcome};
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetCurrentChapter {
    pub chapter_join: ChapterJoin,
    pub current_lifecycle: ProjectLifecycle,
    pub current_chapter_id: Option<String>,
    pub expected_current_chapter_id: String,
    pub target_chapter_id: String,
    pub expected_target_revision_id: String,
    pub current_target_revision_id: String,
}

/// The applied value is the Chapter that becomes the Current Chapter.
pub type SetCurrentChapterResult = TransitionOutcome<
    String,
    SetCurrentChapterNoEffect,
    SetCurrentChapterConflict,
    SetCurrentChapterRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetCurrentChapterNoEffect {
    AlreadyCurrent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetCurrentChapterConflict {
    StaleCurrentChapter,
    WrongTargetHead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetCurrentChapterRefusal {
    ArchivedProject,
    InvalidChapterJoin,
    EmptyProject,
}

reason_codes!(SetCurrentChapterNoEffect { AlreadyCurrent => "already_current" });
reason_codes!(SetCurrentChapterConflict {
    StaleCurrentChapter => "stale_current_chapter",
    WrongTargetHead => "wrong_target_head",
});
reason_codes!(SetCurrentChapterRefusal {
    ArchivedProject => "archived_project",
    InvalidChapterJoin => "invalid_chapter_join",
    EmptyProject => "empty_project",
});

/// Classify one Set Current Chapter against Scope, current Chapter, and target Head.
pub fn set_current_chapter(command: &SetCurrentChapter) -> SetCurrentChapterResult {
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return TransitionOutcome::Refused(SetCurrentChapterRefusal::ArchivedProject);
    }
    if command.chapter_join == ChapterJoin::Invalid || command.target_chapter_id.is_empty() {
        return TransitionOutcome::Refused(SetCurrentChapterRefusal::InvalidChapterJoin);
    }
    let Some(current_chapter_id) = command.current_chapter_id.as_ref() else {
        return TransitionOutcome::Refused(SetCurrentChapterRefusal::EmptyProject);
    };
    if current_chapter_id != &command.expected_current_chapter_id {
        return TransitionOutcome::Conflicted(SetCurrentChapterConflict::StaleCurrentChapter);
    }
    if command.expected_target_revision_id != command.current_target_revision_id {
        return TransitionOutcome::Conflicted(SetCurrentChapterConflict::WrongTargetHead);
    }
    if current_chapter_id == &command.target_chapter_id {
        return TransitionOutcome::NoEffect(SetCurrentChapterNoEffect::AlreadyCurrent);
    }
    TransitionOutcome::Applied(command.target_chapter_id.clone())
}
