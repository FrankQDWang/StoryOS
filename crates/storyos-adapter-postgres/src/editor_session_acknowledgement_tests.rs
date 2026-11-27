use super::{EditorSessionAcknowledgement, decode_editor_session_acknowledgement};

fn acknowledgement(writer: Option<serde_json::Value>) -> String {
    let mut value = serde_json::json!({
        "snapshot_id": "018f0000-0000-7001-8000-000000000022",
        "chapter_id": "018f0000-0000-7001-8000-000000000003",
        "authoritative_revision_id": "018f0000-0000-7001-8000-000000000004",
        "project_activity_position": "7",
        "created_at": "2026-08-13T08:00:00.000Z",
        "author_undo_frontier_sequence": null,
    });
    if let Some(writer) = writer {
        value["writer"] = writer;
    }
    value.to_string()
}

#[test]
fn an_acknowledgement_from_before_adr_0107_decodes_and_its_writer_object_is_ignored() {
    let expected = EditorSessionAcknowledgement {
        snapshot_id: "018f0000-0000-7001-8000-000000000022".to_owned(),
        chapter_id: "018f0000-0000-7001-8000-000000000003".to_owned(),
        authoritative_revision_id: "018f0000-0000-7001-8000-000000000004".to_owned(),
        project_activity_position: 7,
        created_at: "2026-08-13T08:00:00.000Z".to_owned(),
        author_undo_frontier_sequence: None,
    };
    let earlier = serde_json::json!({"kind": "current_writer", "writer_generation": "1"});
    assert_eq!(
        decode_editor_session_acknowledgement(&acknowledgement(None)),
        Ok(expected)
    );
    assert_eq!(
        decode_editor_session_acknowledgement(&acknowledgement(Some(earlier)))
            .map(|ack| ack.project_activity_position),
        Ok(7)
    );
    assert_eq!(
        decode_editor_session_acknowledgement(&acknowledgement(Some(serde_json::json!("1")))),
        Err(())
    );
}
