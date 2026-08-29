//! Pure Core classification for Set Current Chapter.

use super::{ChapterJoin, ProjectLifecycle, ProjectPresence};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetCurrentChapter {
    pub presence: ProjectPresence,
    pub chapter_join: ChapterJoin,
    pub current_lifecycle: ProjectLifecycle,
    pub current_chapter_id: Option<String>,
    pub expected_current_chapter_id: String,
    pub target_chapter_id: String,
    pub expected_target_revision_id: String,
    pub current_target_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetCurrentChapterResult {
    Applied { current_chapter_id: String },
    NoEffect { reason: SetCurrentChapterNoEffect },
    Conflicted { reason: SetCurrentChapterConflict },
    Refused { reason: SetCurrentChapterRefusal },
}

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
    MissingProject,
    ArchivedProject,
    InvalidChapterJoin,
    EmptyProject,
}

/// Classify one Set Current Chapter against Scope, current Chapter, and target Head.
pub fn set_current_chapter(command: &SetCurrentChapter) -> SetCurrentChapterResult {
    if command.presence == ProjectPresence::Absent {
        return SetCurrentChapterResult::Refused {
            reason: SetCurrentChapterRefusal::MissingProject,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return SetCurrentChapterResult::Refused {
            reason: SetCurrentChapterRefusal::ArchivedProject,
        };
    }
    if command.chapter_join == ChapterJoin::Invalid || command.target_chapter_id.is_empty() {
        return SetCurrentChapterResult::Refused {
            reason: SetCurrentChapterRefusal::InvalidChapterJoin,
        };
    }
    let Some(current_chapter_id) = command.current_chapter_id.as_ref() else {
        return SetCurrentChapterResult::Refused {
            reason: SetCurrentChapterRefusal::EmptyProject,
        };
    };
    if current_chapter_id != &command.expected_current_chapter_id {
        return SetCurrentChapterResult::Conflicted {
            reason: SetCurrentChapterConflict::StaleCurrentChapter,
        };
    }
    if command.expected_target_revision_id != command.current_target_revision_id {
        return SetCurrentChapterResult::Conflicted {
            reason: SetCurrentChapterConflict::WrongTargetHead,
        };
    }
    if current_chapter_id == &command.target_chapter_id {
        return SetCurrentChapterResult::NoEffect {
            reason: SetCurrentChapterNoEffect::AlreadyCurrent,
        };
    }
    SetCurrentChapterResult::Applied {
        current_chapter_id: command.target_chapter_id.clone(),
    }
}
