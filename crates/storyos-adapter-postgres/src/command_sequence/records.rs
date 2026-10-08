//! The Admission, Receipt, Project lock, and Command Idempotency Fence rows of the sequence.

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use storyos_core::ProjectLifecycle;
use tokio_postgres::Client;

use super::{LockedProject, ProjectCommand, ReceiptHeads, ReceiptRefs, ReceiptTime, unavailable};

pub(super) struct ReceiptRecord {
    pub(super) result: &'static str,
    pub(super) payload: String,
    pub(super) command_kind: &'static str,
    pub(super) heads: ReceiptHeads,
    pub(super) refs: ReceiptRefs,
    pub(super) revision_ids: Vec<String>,
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
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload,
                source_draft_disposition, created_at)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $11, $6, $7::text::uuid,
                     'author_command_admission', $12::text[]::uuid[], $13::text[]::uuid[],
                     $14::text[]::uuid[], $19::text[]::uuid[], $15::text[]::uuid[],
                     $10::text[]::uuid[], $16::text[], $17::text[], $20::text[], $8,
                     $9::text::jsonb, $18::text::jsonb,
                     CASE WHEN $21 THEN transaction_timestamp() ELSE clock_timestamp() END)
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
                &receipt.refs.proposal_revision_ids,
                &receipt.refs.draft_artifact_refs,
                &receipt.refs.artifact_lifecycle_event_refs,
                &receipt.refs.source_draft_disposition,
                &receipt.revision_ids,
                &receipt.refs.condition_refs,
                &matches!(receipt.refs.created_at, ReceiptTime::Transaction),
            ],
        )
        .await
        .map_err(unavailable)?
        .get::<_, String>(/*idx*/ 0);
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

/// Inserts the Domain Receipt and the child Receipt of a zero-authority outcome.
pub(super) async fn insert_zero_receipt<C: ProjectCommand>(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    receipt: &ReceiptRecord,
) -> Result<String, ProjectCommandError> {
    let receipt_created_at = insert_receipt(client, envelope, receipt, &[]).await?;
    command
        .write_child_receipt(client, envelope, receipt.result)
        .await?;
    Ok(receipt_created_at)
}

/// Inserts the applied Activity record of the Receipt.
///
/// `activity` holds the command fields. The record adds `kind` and the profile fields.
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
    insert_activity_payload(
        client,
        envelope,
        "authoritative_applied",
        activity_kind,
        position,
        event_id,
        serde_json::Value::Object(payload),
    )
    .await
}

/// Inserts one Activity payload of the Receipt with the Receipt result kind that it records.
pub(super) async fn insert_activity_payload(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    receipt_result_kind: &str,
    event_kind: &str,
    position: u64,
    event_id: &str,
    payload: serde_json::Value,
) -> Result<(), ProjectCommandError> {
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position,
                project_activity_event_id, event_kind, receipt_id, receipt_result_kind,
                payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     $5, $6::text::uuid, $7, $8::text::jsonb)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &position.to_string(),
                &event_id,
                &event_kind,
                &envelope.ids.receipt_id,
                &receipt_result_kind,
                &payload.to_string(),
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
    project_row(client, envelope, "FOR UPDATE").await
}

/// Reads the Project row without a lock, for the settle step of a direct editor action. A writer
/// takeover can then settle while an admitted edit waits for its Chapter head.
pub(super) async fn read_project(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<LockedProject, ProjectCommandError> {
    project_row(client, envelope, "").await
}

async fn project_row(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    lock: &str,
) -> Result<LockedProject, ProjectCommandError> {
    let row = client
        .query_opt(
            &format!(
                "SELECT lifecycle_state, tree_revision::text, current_chapter_id::text
                   FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  {lock}"
            ),
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(unavailable)?
        .ok_or(ProjectCommandError::MissingProject)?;
    let lifecycle = match row.get::<_, String>(/*idx*/ 0).as_str() {
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
        tree_revision: row
            .get::<_, String>(/*idx*/ 1)
            .parse()
            .map_err(unavailable)?,
        current_chapter_id: row.get(/*idx*/ 2),
    })
}
