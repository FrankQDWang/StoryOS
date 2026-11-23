//! The earlier turns of a Project Conversation that a decision request with a full input and no
//! prior response sends from the Host records (ADR 0102).

use serde::{Deserialize, Serialize};

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, SelectedProjection, count_context_item_tokens,
};
use crate::{ContextSourceClass, CurrentPassageAssemblyRecord, ProjectionMode};

/// The limit of the earlier turns of one request, in counted tokens (ADR 0052).
pub const EARLIER_TURNS_TOKEN_LIMIT: u64 = 100_000;

/// Whether a message of an earlier turn is a message of the author or the final answer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EarlierMessageKind {
    /// The author message of the Run or a steering input that a RunStep of the Run read.
    Author,
    /// The final answer text of the Run.
    Answer,
}

/// The source of one message of an earlier turn, as the Operation Requirement records it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EarlierTurnSource {
    /// The earlier completed Run of the Conversation.
    pub run_id: String,
    /// The identity of the Run for its first message, of the steering input, or of the Model
    /// Attempt of the final answer. It is the Context Source Version of the selected item.
    pub message_id: String,
    pub message_kind: EarlierMessageKind,
}

/// The earlier turns that the Operation Requirement of one RunStep records, in item order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EarlierTurnsRecord {
    pub sources: Vec<EarlierTurnSource>,
    /// Whether the limit left out one or more older turns.
    pub older_turns_left_out: bool,
}

/// One message of an earlier turn with its exact text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EarlierTurnMessage {
    pub source: EarlierTurnSource,
    pub text: String,
}

/// The messages of one earlier completed Run, in their order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EarlierTurn {
    pub messages: Vec<EarlierTurnMessage>,
}

/// The earlier turns of one request, oldest first.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EarlierTurns {
    pub turns: Vec<EarlierTurn>,
    pub older_turns_left_out: bool,
}

/// Selects the newest of `turns` (oldest first) that fit the limit together. The Host takes whole
/// turns from the newest, and the first turn that does not fit ends the selection. A turn with a
/// message above the Context item limit does not fit (ADR 0052).
pub fn select_earlier_turns(mut turns: Vec<EarlierTurn>) -> EarlierTurns {
    let mut total = 0;
    let mut first_kept = turns.len();
    for (index, turn) in turns.iter().enumerate().rev() {
        let counts: Vec<u64> = (turn.messages.iter())
            .map(|message| count_context_item_tokens(&message.text))
            .collect();
        total += counts.iter().sum::<u64>();
        if total > EARLIER_TURNS_TOKEN_LIMIT
            || counts.iter().any(|count| *count > CONTEXT_ITEM_TOKEN_LIMIT)
        {
            break;
        }
        first_kept = index;
    }
    EarlierTurns {
        turns: turns.split_off(first_kept),
        older_turns_left_out: first_kept > 0,
    }
}

/// Adds one `exact_required` item for each message of `earlier` to `record`, and records their
/// sources in its Operation Requirement. A message of the author has the class
/// `author_instruction`, and a final answer has the class `earlier_answer`.
pub fn assemble_earlier_turns(record: &mut CurrentPassageAssemblyRecord, earlier: &EarlierTurns) {
    let messages: Vec<&EarlierTurnMessage> = (earlier.turns.iter())
        .flat_map(|turn| &turn.messages)
        .collect();
    if messages.is_empty() {
        return;
    }
    for message in &messages {
        let source_class = match message.source.message_kind {
            EarlierMessageKind::Author => ContextSourceClass::AuthorInstruction,
            EarlierMessageKind::Answer => ContextSourceClass::EarlierAnswer,
        };
        let token_count = count_context_item_tokens(&message.text);
        record.considered.push(ConsideredSource {
            source_class,
            source_version: message.source.message_id.clone(),
            token_count,
            eligible: true,
        });
        record.selected.push(SelectedProjection {
            source_class,
            source_version: message.source.message_id.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count,
            content: message.text.clone(),
        });
    }
    record.operation_requirement.earlier_turns = Some(EarlierTurnsRecord {
        sources: messages
            .iter()
            .map(|message| message.source.clone())
            .collect(),
        older_turns_left_out: earlier.older_turns_left_out,
    });
}

/// Whether the selected `item` of `record` is a message of an earlier turn.
pub fn is_earlier_turn(record: &CurrentPassageAssemblyRecord, item: &SelectedProjection) -> bool {
    matches!(
        item.source_class,
        ContextSourceClass::AuthorInstruction | ContextSourceClass::EarlierAnswer
    ) && (record.operation_requirement.earlier_turns.iter())
        .flat_map(|earlier| &earlier.sources)
        .any(|source| source.message_id == item.source_version)
}

/// The earlier turns that `record` selected, oldest first, or `None` when it selected none.
pub fn earlier_turns_of(record: &CurrentPassageAssemblyRecord) -> Option<EarlierTurns> {
    let earlier = record.operation_requirement.earlier_turns.as_ref()?;
    let mut turns: Vec<EarlierTurn> = Vec::new();
    for source in &earlier.sources {
        let text = (record.selected.iter())
            .find(|item| is_earlier_turn(record, item) && item.source_version == source.message_id)?
            .content
            .clone();
        let message = EarlierTurnMessage {
            source: source.clone(),
            text,
        };
        match turns.last_mut() {
            Some(turn) if turn.messages[0].source.run_id == source.run_id => {
                turn.messages.push(message);
            }
            _ => turns.push(EarlierTurn {
                messages: vec![message],
            }),
        }
    }
    Some(EarlierTurns {
        turns,
        older_turns_left_out: earlier.older_turns_left_out,
    })
}

/// Removes the earlier turns from `record`. After a compaction of the Run is installed, the
/// summary and the earlier author messages are in their place (ADR 0093 section 6).
pub fn remove_earlier_turns(record: &mut CurrentPassageAssemblyRecord) {
    let Some(earlier) = record.operation_requirement.earlier_turns.take() else {
        return;
    };
    let earlier_version = |class: ContextSourceClass, version: &str| {
        matches!(
            class,
            ContextSourceClass::AuthorInstruction | ContextSourceClass::EarlierAnswer
        ) && (earlier.sources.iter()).any(|source| source.message_id == version)
    };
    (record.selected).retain(|item| !earlier_version(item.source_class, &item.source_version));
    (record.considered).retain(|item| !earlier_version(item.source_class, &item.source_version));
}

#[cfg(test)]
#[path = "earlier_turns_tests.rs"]
mod tests;
