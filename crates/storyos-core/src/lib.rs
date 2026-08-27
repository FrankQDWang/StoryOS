//! Pure StoryOS Core classification for bounded author commands.

mod archive_project;
mod create_chapter;
mod create_project;
mod create_volume;
mod update_chapter;
mod update_project;
mod update_volume;

pub use archive_project::{
    ArchiveProject, ArchiveProjectConflict, ArchiveProjectNoEffect, ArchiveProjectRefusal,
    ArchiveProjectResult, ProjectLifecycle, archive_project,
};
pub use create_chapter::{
    CreateChapter, CreateChapterConflict, CreateChapterCurrent, CreateChapterOpen,
    CreateChapterRefusal, CreateChapterResult, VolumeJoin, create_chapter,
};
pub use create_project::{CreateProjectResult, ProjectPresence, create_project};
pub use create_volume::{
    CreateVolume, CreateVolumeConflict, CreateVolumeRefusal, CreateVolumeResult, create_volume,
};
pub use update_chapter::{
    ChapterJoin, UpdateChapter, UpdateChapterConflict, UpdateChapterNoEffect, UpdateChapterRefusal,
    UpdateChapterResult, update_chapter,
};
pub use update_project::{
    UpdateProject, UpdateProjectConflict, UpdateProjectNoEffect, UpdateProjectRefusal,
    UpdateProjectResult, update_project,
};
pub use update_volume::{
    UpdateVolume, UpdateVolumeConflict, UpdateVolumeNoEffect, UpdateVolumeRefusal,
    UpdateVolumeResult, update_volume,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyAuthorEdit {
    pub chapter_id: String,
    pub current_authoritative_revision_id: String,
    pub current_body: String,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub current_ownership: CurrentOwnershipFacts,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentOwnershipFacts {
    pub proposal_head_revision_ids: Vec<String>,
    pub anchor_refs: Vec<String>,
    pub unresolved_reservation_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorEditUnit {
    pub normalized_primitives: Vec<AuthorEditPrimitive>,
    pub selection_snapshot: SelectionSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditPrimitive {
    ReplaceSelection { from: u32, to: u32, text: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionSnapshot {
    pub coordinate_profile: String,
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyAuthorEditResult {
    AuthoritativeApplied { body: String },
    Conflicted { reason: AuthorEditConflict },
    NoEffect { reason: AuthorEditNoEffect },
    Refused { reason: AuthorEditRefusal },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditNoEffect {
    ContentUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditConflict {
    StaleAuthoritativeHead,
    ProposalHeadPresent,
    OwnershipChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditRefusal {
    UnsupportedIntentShape,
    InvalidSelection,
    TargetMismatch,
}

pub const UTF16_COORDINATE_PROFILE: &str = "storyos.editor.utf16-code-unit.v1";

pub fn apply_author_edit(command: &ApplyAuthorEdit) -> ApplyAuthorEditResult {
    if command.expected_authoritative_revision_id != command.current_authoritative_revision_id {
        return ApplyAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::StaleAuthoritativeHead,
        };
    }
    if command.expected_proposal_head_revision_ids
        != command.current_ownership.proposal_head_revision_ids
    {
        return ApplyAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::ProposalHeadPresent,
        };
    }
    let current_partition = if command
        .current_ownership
        .proposal_head_revision_ids
        .is_empty()
        && command.current_ownership.anchor_refs.is_empty()
        && command
            .current_ownership
            .unresolved_reservation_refs
            .is_empty()
    {
        "authoritative"
    } else {
        "mixed"
    };
    if command.observed_ownership_partition != current_partition
        || current_partition != "authoritative"
    {
        return ApplyAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::OwnershipChanged,
        };
    }
    if command.target_refs != [format!("manuscript:{}", command.chapter_id)] {
        return ApplyAuthorEditResult::Refused {
            reason: AuthorEditRefusal::TargetMismatch,
        };
    }
    if command.author_edit_units.is_empty() {
        return ApplyAuthorEditResult::Refused {
            reason: AuthorEditRefusal::UnsupportedIntentShape,
        };
    }
    let mut body = command.current_body.clone();
    for unit in &command.author_edit_units {
        let [AuthorEditPrimitive::ReplaceSelection { from, to, text }] =
            unit.normalized_primitives.as_slice()
        else {
            return ApplyAuthorEditResult::Refused {
                reason: AuthorEditRefusal::UnsupportedIntentShape,
            };
        };
        if unit.selection_snapshot.coordinate_profile != UTF16_COORDINATE_PROFILE
            || unit.selection_snapshot.from != *from
            || unit.selection_snapshot.to != *to
        {
            return ApplyAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        }
        let Some(from_byte) = utf16_offset_to_byte(&body, *from) else {
            return ApplyAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        };
        let Some(to_byte) = utf16_offset_to_byte(&body, *to) else {
            return ApplyAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        };
        if from_byte > to_byte {
            return ApplyAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        }
        body = format!("{}{text}{}", &body[..from_byte], &body[to_byte..]);
    }
    if body == command.current_body {
        ApplyAuthorEditResult::NoEffect {
            reason: AuthorEditNoEffect::ContentUnchanged,
        }
    } else {
        ApplyAuthorEditResult::AuthoritativeApplied { body }
    }
}

fn utf16_offset_to_byte(value: &str, wanted: u32) -> Option<usize> {
    if wanted == 0 {
        return Some(0);
    }
    let mut units = 0_u32;
    for (byte, character) in value.char_indices() {
        units += character.len_utf16() as u32;
        if units == wanted {
            return Some(byte + character.len_utf8());
        }
        if units > wanted {
            return None;
        }
    }
    (units == wanted).then_some(value.len())
}

#[cfg(test)]
#[path = "apply_author_edit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "create_project_tests.rs"]
mod create_project_tests;

#[cfg(test)]
#[path = "update_project_tests.rs"]
mod update_project_tests;

#[cfg(test)]
#[path = "archive_project_tests.rs"]
mod archive_project_tests;

#[cfg(test)]
#[path = "create_volume_tests.rs"]
mod create_volume_tests;

#[cfg(test)]
#[path = "create_chapter_tests.rs"]
mod create_chapter_tests;

#[cfg(test)]
#[path = "update_volume_tests.rs"]
mod update_volume_tests;
