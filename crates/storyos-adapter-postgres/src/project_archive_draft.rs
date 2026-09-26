use serde_json::{Value, json};
use storyos_application::{ExportProjectArchiveError, ProjectScope};
use storyos_core::{ArchiveEntrySource, ProjectArchiveBuildRefusal};
use storyos_core::{canonical_json, hex_sha256};

pub(super) fn export_row_expression(table: &str) -> String {
    match table {
        "draft_artifact_revisions" => "CASE WHEN EXISTS (
            SELECT 1 FROM storyos.draft_artifacts AS draft
             WHERE (draft.owner_user_id, draft.project_id, draft.draft_id) =
                   (source.owner_user_id, source.project_id, source.draft_id)
               AND draft.retention_state = 'tombstoned')
          THEN (to_jsonb(source) - 'payload') || jsonb_build_object('payload_availability', jsonb_build_object(
            'kind', 'refused_edit_revision_payload', 'reason', 'withheld_due_to_tombstone',
            'entry_path', 'canonical/draft_artifact_revisions.json', 'record_id', source.revision_id,
            'payload_field', 'payload', 'draft_id', source.draft_id, 'retention_state', 'tombstoned',
            'payload_digest', source.payload_digest, 'payload_digest_profile', source.payload_digest_profile))
          ELSE to_jsonb(source) END".to_owned(),
        "author_command_admissions" => "CASE WHEN EXISTS (
            SELECT 1 FROM storyos.draft_lifecycle_events AS event
              JOIN storyos.draft_artifacts AS draft USING (owner_user_id, project_id, draft_id)
             WHERE (event.owner_user_id, event.project_id, event.author_command_admission_id, event.command_id) =
                   (source.owner_user_id, source.project_id, source.author_command_admission_id, source.command_id)
               AND draft.retention_state = 'tombstoned')
          THEN (to_jsonb(source) - 'command_payload') || jsonb_build_object('payload_availability', jsonb_build_object(
            'kind', 'refused_edit_admission_payload', 'reason', 'withheld_due_to_tombstone',
            'entry_path', 'canonical/author_command_admissions.json', 'record_id', source.author_command_admission_id,
            'payload_field', 'command_payload', 'draft_id', (SELECT event.draft_id FROM storyos.draft_lifecycle_events AS event
              WHERE (event.owner_user_id, event.project_id, event.author_command_admission_id, event.command_id) =
                    (source.owner_user_id, source.project_id, source.author_command_admission_id, source.command_id)),
            'retention_state', 'tombstoned', 'command_id', source.command_id,
            'canonical_command_digest', source.canonical_command_digest))
          ELSE to_jsonb(source) END".to_owned(),
        _ => "to_jsonb(source)".to_owned(),
    }
}

pub(super) fn withheld_payload_gaps(
    sources: &[ArchiveEntrySource],
) -> Result<Vec<Value>, ProjectArchiveBuildRefusal> {
    let draft_family_count = [
        "draft_artifacts",
        "draft_artifact_revisions",
        "draft_lifecycle_events",
    ]
    .iter()
    .filter(|table| {
        sources
            .iter()
            .any(|source| source.path == format!("canonical/{table}.json"))
    })
    .count();
    if draft_family_count == 0 {
        return Ok(Vec::new());
    }
    if draft_family_count != 3 {
        return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
    }
    let rows = |table| {
        let path = format!("canonical/{table}.json");
        let source = sources
            .iter()
            .find(|source| source.path == path)
            .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)?;
        serde_json::from_slice::<Vec<Value>>(&source.bytes)
            .map_err(|_| ProjectArchiveBuildRefusal::InvalidProvenance)
    };
    let drafts = rows("draft_artifacts")?;
    let revisions = rows("draft_artifact_revisions")?;
    let events = rows("draft_lifecycle_events")?;
    let admissions = rows("author_command_admissions")?;
    let receipts = rows("domain_receipts")?;
    let mut gaps = Vec::new();
    let mut withheld_revisions = 0;
    let mut withheld_admissions = 0;
    for draft in &drafts {
        if draft["retention_state"] != "tombstoned" {
            continue;
        }
        let draft_id = text(draft, "draft_id")?;
        let revision_id = text(draft, "current_revision_id")?;
        let selected_revisions = revisions
            .iter()
            .filter(|row| row["draft_id"] == draft_id && row["revision_id"] == revision_id)
            .collect::<Vec<_>>();
        let [revision] = selected_revisions.as_slice() else {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        };
        let revision = *revision;
        let digest = text(revision, "payload_digest")?;
        if revision.get("payload").is_some()
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || revision["payload_digest_profile"] != "storyos.refused-edit-payload.jcs.v1"
        {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        }
        let selected_events = events
            .iter()
            .filter(|row| row["draft_id"] == draft_id && row["revision_id"] == revision_id)
            .collect::<Vec<_>>();
        let [event] = selected_events.as_slice() else {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        };
        let admission_id = text(event, "author_command_admission_id")?;
        let command_id = text(event, "command_id")?;
        let receipt_id = text(event, "receipt_id")?;
        let selected_admissions = admissions
            .iter()
            .filter(|row| {
                row["author_command_admission_id"] == admission_id
                    && row["command_id"] == command_id
            })
            .collect::<Vec<_>>();
        let [admission] = selected_admissions.as_slice() else {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        };
        let command_digest = text(admission, "canonical_command_digest")?;
        let selected_receipts = receipts
            .iter()
            .filter(|row| {
                row["receipt_id"] == receipt_id
                    && row["author_command_admission_id"] == admission_id
                    && row["command_id"] == command_id
            })
            .collect::<Vec<_>>();
        let [receipt] = selected_receipts.as_slice() else {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        };
        if admission.get("command_payload").is_some()
            || receipt["command_digest"] != command_digest
            || receipt["result_kind"] != "refused_to_draft"
            || receipt["draft_artifact_refs"] != json!([draft_id])
            || receipt["artifact_lifecycle_event_refs"]
                != json!([text(event, "creation_event_id")?])
            || receipt["result_payload"]["draft_revision_id"] != revision_id
        {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        }
        let command_hex = command_digest
            .strip_prefix("sha256:storyos.command.applyAuthorEdit.jcs.v1:")
            .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)?;
        if command_hex.len() != 64
            || !command_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        }
        let revision_gap = json!({"kind": "refused_edit_revision_payload", "reason": "withheld_due_to_tombstone",
            "entry_path": "canonical/draft_artifact_revisions.json", "record_id": revision_id,
            "payload_field": "payload", "draft_id": draft_id, "retention_state": "tombstoned",
            "payload_digest": digest, "payload_digest_profile": "storyos.refused-edit-payload.jcs.v1"});
        let admission_gap = json!({"kind": "refused_edit_admission_payload", "reason": "withheld_due_to_tombstone",
            "entry_path": "canonical/author_command_admissions.json", "record_id": admission_id,
            "payload_field": "command_payload", "draft_id": draft_id, "retention_state": "tombstoned",
            "command_id": command_id, "canonical_command_digest": command_digest});
        if revision.get("payload_availability") != Some(&revision_gap)
            || admission.get("payload_availability") != Some(&admission_gap)
        {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        }
        gaps.extend([revision_gap, admission_gap]);
        withheld_revisions += 1;
        withheld_admissions += 1;
    }
    if revisions
        .iter()
        .filter(|row| row.get("payload").is_none())
        .count()
        != withheld_revisions
        || admissions
            .iter()
            .filter(|row| row.get("command_payload").is_none())
            .count()
            != withheld_admissions
    {
        return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
    }
    gaps.sort_by_cached_key(storyos_core::canonical_json);
    Ok(gaps)
}

fn text<'a>(row: &'a Value, field: &str) -> Result<&'a str, ProjectArchiveBuildRefusal> {
    row.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)
}

pub(super) async fn validate_withheld_payloads(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<(), ExportProjectArchiveError> {
    let rows = client.query(
        "SELECT revision.payload::text, revision.payload_digest,
                admission.command_payload::text, admission.canonical_command_digest,
                receipt.command_digest
           FROM storyos.draft_artifacts AS draft
           LEFT JOIN storyos.draft_artifact_revisions AS revision
             ON (revision.owner_user_id, revision.project_id, revision.draft_id, revision.revision_id) =
                (draft.owner_user_id, draft.project_id, draft.draft_id, draft.current_revision_id)
           LEFT JOIN storyos.draft_lifecycle_events AS event
             ON (event.owner_user_id, event.project_id, event.draft_id, event.revision_id) =
                (draft.owner_user_id, draft.project_id, draft.draft_id, draft.current_revision_id)
           LEFT JOIN storyos.author_command_admissions AS admission
             ON (admission.owner_user_id, admission.project_id, admission.author_command_admission_id, admission.command_id) =
                (event.owner_user_id, event.project_id, event.author_command_admission_id, event.command_id)
           LEFT JOIN storyos.domain_receipts AS receipt
             ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id, receipt.author_command_admission_id,
                 receipt.command_id, receipt.result_kind) =
                (event.owner_user_id, event.project_id, event.receipt_id, event.author_command_admission_id,
                 event.command_id, event.receipt_result_kind)
          WHERE draft.owner_user_id=$1::text::uuid AND draft.project_id=$2::text::uuid
            AND draft.retention_state='tombstoned'",
        &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
    ).await.map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
    for row in rows {
        let invalid = || {
            ExportProjectArchiveError::ArchiveBuild(ProjectArchiveBuildRefusal::InvalidProvenance)
        };
        let payload: Value =
            serde_json::from_str(&row.get::<_, Option<String>>(0).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
        let payload_digest = row.get::<_, Option<String>>(1).ok_or_else(invalid)?;
        let command: Value =
            serde_json::from_str(&row.get::<_, Option<String>>(2).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
        let command_digest = row.get::<_, Option<String>>(3).ok_or_else(invalid)?;
        let receipt_digest = row.get::<_, Option<String>>(4).ok_or_else(invalid)?;
        if hex_sha256(canonical_json(&payload).as_bytes()) != payload_digest
            || command_digest != receipt_digest
            || command_digest
                != format!(
                    "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
                    hex_sha256(canonical_json(&command).as_bytes())
                )
        {
            return Err(invalid());
        }
    }
    Ok(())
}
