use storyos_application::{EditorReadOnlyReason, EditorSession, EditorWriterState};

/// The facts of a first Create Editor Session acknowledgement that later commands can change.
///
/// The fence keeps them in `command_idempotency.response_editor_session`. The Editor Session row
/// keeps the client binding and the open time. The Authoritative Revision keeps the body.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EditorSessionAcknowledgement {
    pub(crate) snapshot_id: String,
    pub(crate) chapter_id: String,
    pub(crate) authoritative_revision_id: String,
    pub(crate) project_activity_position: u64,
    pub(crate) created_at: String,
    pub(crate) writer: EditorWriterState,
    pub(crate) author_undo_frontier_sequence: Option<u64>,
}

pub(crate) fn encode_editor_session_acknowledgement(session: &EditorSession) -> String {
    let writer = match &session.writer {
        EditorWriterState::CurrentWriter { writer_generation } => serde_json::json!({
            "kind": "current_writer",
            "writer_generation": writer_generation.to_string(),
        }),
        EditorWriterState::ReadOnly {
            observed_writer_generation,
            reason,
        } => serde_json::json!({
            "kind": "read_only",
            "observed_writer_generation": observed_writer_generation.to_string(),
            "reason": match reason {
                EditorReadOnlyReason::SecondarySession => "secondary_session",
                EditorReadOnlyReason::SupersededByTakeover => "superseded_by_takeover",
                EditorReadOnlyReason::BindingInvalid => "binding_invalid",
            },
        }),
    };
    let snapshot = &session.base_snapshot;
    serde_json::json!({
        "snapshot_id": snapshot.snapshot_id,
        "chapter_id": snapshot.chapter_id,
        "authoritative_revision_id": snapshot.authoritative_revision_id,
        "project_activity_position": snapshot.project_activity_position.to_string(),
        "created_at": snapshot.created_at,
        "writer": writer,
        "author_undo_frontier_sequence": session
            .author_undo_frontier_sequence
            .map(|sequence| sequence.to_string()),
    })
    .to_string()
}

/// Decodes a stored acknowledgement. Any other shape is damaged evidence.
pub(crate) fn decode_editor_session_acknowledgement(
    payload: &str,
) -> Result<EditorSessionAcknowledgement, ()> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    if object.len() != 7 {
        return Err(());
    }
    let text = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or(())
    };
    let writer = object
        .get("writer")
        .and_then(serde_json::Value::as_object)
        .ok_or(())?;
    let writer_text = |key: &str| {
        writer
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or(())
    };
    let writer = match (writer_text("kind")?, writer.len()) {
        ("current_writer", 2) => EditorWriterState::CurrentWriter {
            writer_generation: decimal(writer_text("writer_generation")?)?,
        },
        ("read_only", 3) => EditorWriterState::ReadOnly {
            observed_writer_generation: decimal(writer_text("observed_writer_generation")?)?,
            reason: match writer_text("reason")? {
                "secondary_session" => EditorReadOnlyReason::SecondarySession,
                "superseded_by_takeover" => EditorReadOnlyReason::SupersededByTakeover,
                "binding_invalid" => EditorReadOnlyReason::BindingInvalid,
                _ => return Err(()),
            },
        },
        _ => return Err(()),
    };
    let author_undo_frontier_sequence = match object.get("author_undo_frontier_sequence") {
        Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(sequence)) => Some(decimal(sequence)?),
        _ => return Err(()),
    };
    Ok(EditorSessionAcknowledgement {
        snapshot_id: text("snapshot_id")?.to_owned(),
        chapter_id: text("chapter_id")?.to_owned(),
        authoritative_revision_id: text("authoritative_revision_id")?.to_owned(),
        project_activity_position: decimal(text("project_activity_position")?)?,
        created_at: text("created_at")?.to_owned(),
        writer,
        author_undo_frontier_sequence,
    })
}

/// An unsigned decimal in its canonical text form.
fn decimal(text: &str) -> Result<u64, ()> {
    text.parse::<u64>()
        .ok()
        .filter(|value| value.to_string() == text)
        .ok_or(())
}
