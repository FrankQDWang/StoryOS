//! The earlier messages of the author that the later request of a compaction sends in their exact
//! words, before the current input (ADR 0093 section 6).

use serde::{Deserialize, Serialize};

use crate::assemble_context::{ConsideredSource, SelectedProjection, count_context_item_tokens};
use crate::{ContextSourceClass, CurrentPassageAssemblyRecord, ProjectionMode};

/// The limit of the earlier author messages of one later request, in counted tokens (ADR 0052).
pub const EARLIER_AUTHOR_MESSAGES_TOKEN_LIMIT: u64 = 20_000;

/// The source of one earlier author message, as the Operation Requirement of the later request
/// records it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EarlierAuthorMessageSource {
    /// The earlier Run of the Conversation that read the message.
    pub run_id: String,
    /// The identity of the Run for its first message, or the identity of the steering input. It
    /// is the Context Source Version of the selected item.
    pub message_id: String,
}

/// One author message or steering input of an earlier Run of the Conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EarlierAuthorMessage {
    pub source: EarlierAuthorMessageSource,
    pub text: String,
}

/// Puts the newest of `messages` (oldest first) that fit the limit together into the later
/// request `record`, oldest first, after its summary and before the current input. The Host takes
/// whole messages from the newest, and the first message that does not fit ends the selection.
pub fn keep_earlier_author_messages(
    record: &mut CurrentPassageAssemblyRecord,
    messages: &[EarlierAuthorMessage],
) {
    let mut total = 0;
    let mut first_kept = messages.len();
    for (index, message) in messages.iter().enumerate().rev() {
        total += count_context_item_tokens(&message.text);
        if total > EARLIER_AUTHOR_MESSAGES_TOKEN_LIMIT {
            break;
        }
        first_kept = index;
    }
    let kept = &messages[first_kept..];
    if kept.is_empty() {
        return;
    }
    let considered_at = after_summary(record.considered.iter().map(|item| item.source_class));
    let selected_at = after_summary(record.selected.iter().map(|item| item.source_class));
    record.considered.splice(
        considered_at..considered_at,
        kept.iter().map(|message| ConsideredSource {
            source_class: ContextSourceClass::AuthorInstruction,
            source_version: message.source.message_id.clone(),
            token_count: count_context_item_tokens(&message.text),
            eligible: true,
        }),
    );
    record.selected.splice(
        selected_at..selected_at,
        kept.iter().map(|message| SelectedProjection {
            source_class: ContextSourceClass::AuthorInstruction,
            source_version: message.source.message_id.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count: count_context_item_tokens(&message.text),
            content: message.text.clone(),
        }),
    );
    record.operation_requirement.earlier_author_messages =
        Some(kept.iter().map(|message| message.source.clone()).collect());
}

/// The index after the summary item, or 0 when `classes` has no summary.
fn after_summary(mut classes: impl Iterator<Item = ContextSourceClass>) -> usize {
    classes
        .position(|class| class == ContextSourceClass::ConversationSummary)
        .map_or(0, |index| index + 1)
}

/// Whether the selected `item` of `record` is an earlier author message, not the current input.
pub fn is_earlier_author_message(
    record: &CurrentPassageAssemblyRecord,
    item: &SelectedProjection,
) -> bool {
    item.source_class == ContextSourceClass::AuthorInstruction
        && record
            .operation_requirement
            .earlier_author_messages
            .iter()
            .flatten()
            .any(|source| source.message_id == item.source_version)
}

#[cfg(test)]
#[path = "earlier_author_messages_tests.rs"]
mod tests;
