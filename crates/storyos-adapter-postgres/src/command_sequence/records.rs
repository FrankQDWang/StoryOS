//! The Admission, Receipt, Project lock, and Command Idempotency Fence rows of the sequence.

use storyos_application::{ChapterId, Project, ProjectCommandEnvelope, ProjectCommandError};
use storyos_core::ProjectLifecycle;
use tokio_postgres::Client;

use super::{CommandSpec, LockedProject, ReceiptHeads, ResponseRecord, unavailable};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};

pub(super) struct ReceiptRecord {
    pub(super) result: &'static str,
    pub(super) payload: String,
    pub(super) command_kind: &'static str,
    pub(super) heads: ReceiptHeads,
}

/// Inserts the Domain Receipt and its Admission settlement link.
pub(super) async fn insert_receipt(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    receipt: &ReceiptRecord,
    commit_ids: &[String],
) -> Result<String, ProjectCommandError> {
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $11, $6, $7::text::uuid,
                     'author_command_admission', $12::text[]::uuid[], $13::text[]::uuid[],
                     $14::text[]::uuid[], '{}'::uuid[], '{}'::uuid[], $10::text[]::uuid[],
                     '{}'::text[], '{}'::text[], '{}'::text[], $8, $9::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.ids.author_command_admission_id,
                &envelope.ids.command_id,
                &envelope.challenge_binding.canonical_command_digest,
                &envelope.challenge_binding.idempotency_key,
                &receipt.result,
                &receipt.payload,
                &commit_ids,
                &receipt.command_kind,
                &receipt.heads.expected,
                &receipt.heads.prior,
                &receipt.heads.resulting,
            ],
        )
        .await
        .map_err(unavailable)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.author_command_admission_id,
                &envelope.ids.receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(receipt_created_at)
}

/// Inserts the applied Activity record of the Receipt.
///
/// `activity` holds the command fields; the record adds `kind` and the profile fields.
pub(super) async fn insert_applied_activity(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    activity_kind: &'static str,
    position: u64,
    event_id: &str,
    activity: serde_json::Value,
    profile_fields: &[(&str, String)],
) -> Result<(), ProjectCommandError> {
    let serde_json::Value::Object(mut payload) = activity else {
        return Err(unavailable("the Activity payload is not an object"));
    };
    payload.insert("kind".to_owned(), activity_kind.into());
    for (key, value) in profile_fields {
        payload.insert((*key).to_owned(), value.as_str().into());
    }
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position,
                project_activity_event_id, event_kind, receipt_id, receipt_result_kind,
                payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     $5, $6::text::uuid, 'authoritative_applied', $7::text::jsonb)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &position.to_string(),
                &event_id,
                &activity_kind,
                &envelope.ids.receipt_id,
                &serde_json::Value::Object(payload).to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

pub(super) async fn lock_project(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<LockedProject, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state, tree_revision::text, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(unavailable)?
        .ok_or(ProjectCommandError::MissingProject)?;
    let lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(unavailable(format!(
                "unsupported Project lifecycle {other}"
            )));
        }
    };
    Ok(LockedProject {
        lifecycle,
        tree_revision: row.get::<_, String>(1).parse().map_err(unavailable)?,
        current_chapter_id: row.get(2),
    })
}

/// Reads the Command-response Project after the writes and settles the Command Idempotency Fence.
pub(super) async fn settle_idempotency(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    spec: &CommandSpec,
) -> Result<Project, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let row = client
        .query_one(
            "SELECT title, current_chapter_id::text FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?;
    let project = Project {
        project_id: scope.project_id.clone(),
        title: row.get(0),
        current_chapter_id: row.get::<_, Option<String>>(1).map(ChapterId::new),
    };
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled',
                    result_reference = $3,
                    acknowledgement_format = $5,
                    response_project = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = $7 AND idempotency_key = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.challenge_binding.idempotency_key,
                &match spec.response {
                    ResponseRecord::Project => COMMAND_RESPONSE_PROJECT_FORMAT,
                },
                &encode_command_response_project(&project),
                &spec.kind,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(project)
}
