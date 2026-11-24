//! The shared test support from `chapter_list_context_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_structure::{SET_CURRENT_CHAPTER, UPDATE_CHAPTER};
use crate::shared_tests::command_sequence_support::{CommandCall, issued};
use crate::shared_tests::create_volume_authority::{NamedEdit, adjusted_edit_command};
use crate::shared_tests::model_gateway::Admission;
use crate::statement_cache::Client;
use storyos_application::{
    ApplyAuthorEditCommand, ChallengeRateClass, ChapterId, ConversationSelection, EditorSessionId,
    ProjectScope, SetCurrentChapterInput, UpdateChapterInput,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot, UTF16_COORDINATE_PROFILE,
};

/// Admits one Run of the tool loop profile with `message` in `conversation` of the Project.
pub(crate) async fn admitted(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    message: &str,
    conversation: ConversationSelection,
) -> String {
    Admission {
        message,
        conversation,
        ..Default::default()
    }
    .admit(store, scope, chapter_id, prefix)
    .await
}

/// The rename of the fixture Chapter to `title`, with a fresh challenge `suffix`.
pub(crate) async fn rename(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    chapter_id: &str,
    title: &str,
) -> CommandCall<UpdateChapterInput> {
    let input = UpdateChapterInput {
        chapter_id: ChapterId::new(chapter_id),
        title: title.to_owned(),
        order: 1,
        expected_tree_revision: 3,
    };
    issued(store, scope, suffix, &UPDATE_CHAPTER, input).await
}

/// The head of `chapter_id` and its first Block.
pub(crate) async fn head(admin: &Client, chapter_id: &str) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT head.current_revision_id::text, member.manuscript_block_id::text
               FROM storyos.authoritative_heads AS head
               JOIN storyos.manuscript_revision_members AS member
                 ON member.revision_id = head.current_revision_id
              WHERE head.manuscript_object_id = $1::text::uuid
              ORDER BY member.block_order
              LIMIT 1",
            &[&chapter_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// The Author Edit of the Editor Session `editor_session_id` that replaces the first `length`
/// characters of the head of `chapter_id` with `text`. `sequence` is its local intent sequence,
/// and `suffix` is its fresh challenge suffix.
pub(crate) async fn replace_text(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    (editor_session_id, sequence, suffix): (&str, u64, &str),
    chapter_id: &str,
    (length, text): (u32, &str),
) -> ApplyAuthorEditCommand {
    let (revision, block) = head(admin, chapter_id).await;
    let edit = NamedEdit {
        editor_session_id,
        chapter_id,
        expected_revision_id: &revision,
        suffix,
        local_intent_sequence: sequence,
        text: "",
        proposal_target: None,
    };
    adjusted_edit_command(store, scope, edit, |command| {
        // The Author Edit class keeps the Shared Challenges of the Project for the Runs.
        command.challenge_binding.challenge_rate_policy_revision =
            ChallengeRateClass::AuthorEdit.policy_revision().to_owned();
        command.author_edit_units = vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: block,
                from: 0,
                to: length,
                text: text.to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: 0,
                to: length,
            },
        }];
    })
    .await
}

/// The selection of `chapter_id` as the Current Chapter of the Project in place of `from`, by
/// the Editor Session `editor_session_id`, with a fresh challenge `suffix`. An Author Edit
/// changes only the Current Chapter of the Project.
pub(crate) async fn select(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    (editor_session_id, suffix): (&EditorSessionId, u16),
    chapter_id: &str,
    from: &str,
) -> CommandCall<SetCurrentChapterInput> {
    let input = SetCurrentChapterInput {
        editor_session_id: editor_session_id.clone(),
        chapter_id: chapter_id.to_owned(),
        expected_current_chapter_id: from.to_owned(),
        expected_target_revision_id: head(admin, chapter_id).await.0,
    };
    issued(store, scope, suffix, &SET_CURRENT_CHAPTER, input).await
}
