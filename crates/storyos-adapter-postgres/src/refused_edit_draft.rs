use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, ProjectReadError,
    ProjectScope, RefusedEditDraftClosure, RefusedEditDraftIdentity, RefusedEditDraftReader,
    RefusedEditDraftRecord,
};
use storyos_core::{RefusedEditPayload, canonical_json, hex_sha256};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::author_edit::author_edit_database_error;
use crate::{PostgresProjectReader, read_error, set_challenge_scope_on_client};

/// New identities for one Refused Edit Draft.
pub(super) fn new_identity() -> RefusedEditDraftIdentity {
    RefusedEditDraftIdentity {
        draft_id: Uuid::now_v7().to_string(),
        draft_revision_id: Uuid::now_v7().to_string(),
        creation_event_id: Uuid::now_v7().to_string(),
    }
}

/// Writes the Refused Edit Draft of `command` with the identities of `identity`.
pub(super) async fn persist(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    identity: RefusedEditDraftIdentity,
) -> Result<RefusedEditDraftIdentity, AuthorEditError> {
    let payload = serde_json::to_value(RefusedEditPayload {
        schema_revision: "storyos.refused-edit-payload.v1".to_owned(),
        chapter_id: command.chapter_id.clone(),
        expected_authoritative_revision_id: command.expected_authoritative_revision_id.clone(),
        expected_proposal_head_revision_ids: command.expected_proposal_head_revision_ids.clone(),
        target_refs: command.target_refs.clone(),
        author_edit_units: command.author_edit_units.clone(),
        undo_group_id: command.undo_group_id.clone(),
        completed_intent_record_id: command.completed_intent_record_id.clone(),
        local_intent_sequence: command.local_intent_sequence.to_string(),
    })
    .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    let digest = hex_sha256(canonical_json(&payload).as_bytes());
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    client.execute(
        "INSERT INTO storyos.draft_artifacts (owner_user_id, project_id, draft_id, current_revision_id)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
        &[&owner, &project, &identity.draft_id, &identity.draft_revision_id],
    ).await.map_err(author_edit_database_error)?;
    client.execute(
        "INSERT INTO storyos.draft_artifact_revisions
           (owner_user_id, project_id, draft_id, revision_id, payload, payload_digest)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::jsonb, $6)",
        &[&owner, &project, &identity.draft_id, &identity.draft_revision_id, &canonical_json(&payload), &digest],
    ).await.map_err(author_edit_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.draft_lifecycle_events
           (owner_user_id, project_id, creation_event_id, draft_id, revision_id,
            receipt_id, author_command_admission_id, command_id)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid,
                 $6::text::uuid, $7::text::uuid, $8::text::uuid)",
            &[
                &owner,
                &project,
                &identity.creation_event_id,
                &identity.draft_id,
                &identity.draft_revision_id,
                &command.ids.receipt_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    Ok(identity)
}

impl RefusedEditDraftReader for PostgresProjectReader {
    async fn read_refused_edit_draft(
        &self,
        scope: &ProjectScope,
        draft_id: &str,
    ) -> Result<Option<RefusedEditDraftRecord>, ProjectReadError> {
        let transaction = self.connect().await?;
        transaction
            .batch_execute("BEGIN READ ONLY")
            .await
            .map_err(read_error)?;
        set_challenge_scope_on_client(&transaction, scope)
            .await
            .map_err(ProjectReadError::unavailable)?;
        let row = transaction.query_opt(
            "SELECT draft.draft_id::text, revision.revision_id::text, event.creation_event_id::text,
                    revision.payload::text, revision.payload_digest, event.command_id::text,
                    event.author_command_admission_id::text, event.receipt_id::text,
                    receipt.command_digest, receipt.idempotency_key::text,
                    to_char(revision.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                    draft.closure, draft.retention_state,
                    closed.event_id::text, closed_receipt.command_id::text,
                    closed_receipt.author_command_admission_id::text, closed.receipt_id::text,
                    closed_receipt.command_digest, closed_receipt.idempotency_key::text,
                    closed.author_action_sequence::text,
                    to_char(closed.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'), reopened.payload::text, closed.close_reason, receipt.source_draft_disposition::text,
                    (admission.command_payload->'retry_source')::text
               FROM storyos.draft_artifacts AS draft
               JOIN storyos.projects AS project USING (owner_user_id, project_id)
               JOIN storyos.draft_artifact_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.draft_id, revision.revision_id) =
                    (draft.owner_user_id, draft.project_id, draft.draft_id, draft.current_revision_id)
               JOIN storyos.draft_lifecycle_events AS event
                 ON (event.owner_user_id, event.project_id, event.draft_id, event.revision_id) =
                    (revision.owner_user_id, revision.project_id, revision.draft_id, revision.revision_id)
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (event.owner_user_id, event.project_id, event.receipt_id)
               JOIN storyos.author_command_admissions AS admission ON
                 (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=
                 (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
               LEFT JOIN storyos.draft_close_events AS closed
                 ON (closed.owner_user_id,closed.project_id,closed.event_id,closed.draft_id,closed.revision_id)=
                    (draft.owner_user_id,draft.project_id,draft.close_event_id,draft.draft_id,draft.current_revision_id)
               LEFT JOIN storyos.draft_reopen_events AS reopened ON
                 (reopened.owner_user_id,reopened.project_id,reopened.event_id)=
                 (draft.owner_user_id,draft.project_id,draft.reopen_event_id)
               LEFT JOIN storyos.domain_receipts AS closed_receipt
                 ON (closed_receipt.owner_user_id,closed_receipt.project_id,closed_receipt.receipt_id)=
                    (closed.owner_user_id,closed.project_id,closed.receipt_id)
              WHERE draft.owner_user_id = $1::text::uuid AND draft.project_id = $2::text::uuid
                AND draft.draft_id = $3::text::uuid AND project.lifecycle_state = 'active'
                AND draft.retention_state = 'retained'",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref(), &draft_id],
        ).await.map_err(read_error)?;
        let record = row
            .map(|row| {
                let value: serde_json::Value =
                    serde_json::from_str(row.get::<_, String>(3).as_str())
                        .map_err(ProjectReadError::unavailable)?;
                let digest: String = row.get(4);
                if digest != hex_sha256(canonical_json(&value).as_bytes()) {
                    return Err(ProjectReadError::unavailable(std::io::Error::other(
                        "Draft payload digest mismatch",
                    )));
                }
                let disposition: Option<storyos_contracts::SourceDraftDisposition> = row
                    .get::<_, Option<String>>(23)
                    .map(|value| serde_json::from_str(&value))
                    .transpose()
                    .map_err(ProjectReadError::unavailable)?;
                let retry: Option<storyos_contracts::DraftRetry> = row
                    .get::<_, Option<String>>(24)
                    .map(|value| serde_json::from_str(&value))
                    .transpose()
                    .map_err(ProjectReadError::unavailable)?;
                Ok(RefusedEditDraftRecord {
                    replacement_provenance: crate::draft_retry::replacement_provenance(
                        disposition.as_ref(),
                        retry.as_ref(),
                    ),
                    project_scope: scope.clone(),
                    identity: RefusedEditDraftIdentity {
                        draft_id: row.get(0),
                        draft_revision_id: row.get(1),
                        creation_event_id: row.get(2),
                    },
                    payload: serde_json::from_value(value)
                        .map_err(ProjectReadError::unavailable)?,
                    payload_digest: digest,
                    source: AuthorCommandAdmissionIds {
                        command_id: row.get(5),
                        author_command_admission_id: row.get(6),
                        receipt_id: row.get(7),
                    },
                    command_digest: row.get(8),
                    idempotency_key: row.get(9),
                    created_at: row.get(10),
                    reopen_event: row
                        .get::<_, Option<String>>(21)
                        .map(|payload| serde_json::from_str(&payload))
                        .transpose()
                        .map_err(ProjectReadError::unavailable)?,
                    closure: row.get(11),
                    retention: row.get(12),
                    closure_event: row.get::<_, Option<String>>(13).map(|event_id| {
                        RefusedEditDraftClosure {
                            event_id,
                            source: AuthorCommandAdmissionIds {
                                command_id: row.get(14),
                                author_command_admission_id: row.get(15),
                                receipt_id: row.get(16),
                            },
                            command_digest: row.get(17),
                            idempotency_key: row.get(18),
                            author_action_sequence: row.get(19),
                            close_reason: row.get(22),
                            created_at: row.get(20),
                        }
                    }),
                })
            })
            .transpose()?;
        transaction
            .batch_execute("COMMIT")
            .await
            .map_err(read_error)?;
        Ok(record)
    }
}
