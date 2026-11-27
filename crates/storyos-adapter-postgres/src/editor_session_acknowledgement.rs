use storyos_application::EditorSession;

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
    pub(crate) author_undo_frontier_sequence: Option<u64>,
}

pub(crate) fn encode_editor_session_acknowledgement(session: &EditorSession) -> String {
    let snapshot = &session.base_snapshot;
    serde_json::json!({
        "snapshot_id": snapshot.snapshot_id,
        "chapter_id": snapshot.chapter_id,
        "authoritative_revision_id": snapshot.authoritative_revision_id,
        "project_activity_position": snapshot.project_activity_position.to_string(),
        "created_at": snapshot.created_at,
        "author_undo_frontier_sequence": session
            .author_undo_frontier_sequence
            .map(|sequence| sequence.to_string()),
    })
    .to_string()
}

/// Decodes a stored acknowledgement. Any other shape is damaged evidence.
///
/// An acknowledgement from before ADR 0107 also has a `writer` object, which the decoder ignores.
pub(crate) fn decode_editor_session_acknowledgement(
    payload: &str,
) -> Result<EditorSessionAcknowledgement, ()> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    let earlier_writer = object.get("writer").map(serde_json::Value::is_object);
    if earlier_writer == Some(false) || object.len() != 6 + usize::from(earlier_writer.is_some()) {
        return Err(());
    }
    let text = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or(())
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

#[cfg(test)]
#[path = "editor_session_acknowledgement_tests.rs"]
mod tests;
