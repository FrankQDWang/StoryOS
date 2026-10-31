//! The Admission, Receipt, Project lock, scope counter, and Command Idempotency Fence rows of the
//! sequence.

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use tokio_postgres::Client;

use super::{ActionDisposition, LockedProject, ReceiptHeads, ReceiptRefs, unavailable};

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
                created_at)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $11, $6, $7::text::uuid,
                     'author_command_admission', $12::text[]::uuid[], $13::text[]::uuid[],
                     $14::text[]::uuid[], $16::text[]::uuid[], $15::text[]::uuid[],
                     $10::text[]::uuid[], '{}'::text[], '{}'::text[], $17::text[], $8,
                     $9::text::jsonb, clock_timestamp())
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
                &receipt.revision_ids,
                &receipt.refs.condition_refs,
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

/// The scope sequences of one applied outcome with an Authoritative Commit.
pub(crate) struct CommitCounters {
    pub(crate) author_action_sequence: u64,
    pub(crate) authoritative_commit_sequence: u64,
    pub(crate) project_activity_position: u64,
}

/// Advances the Author Action, Authoritative Commit, and Project Activity counters of the scope.
/// `Structural` and `AuthoritativeRevision` share this allocation.
pub(crate) async fn allocate_commit_counters(
    client: &Client,
    scope: &ProjectScope,
) -> Result<CommitCounters, ProjectCommandError> {
    let row = client
        .query_one(
            "INSERT INTO storyos.scope_counters AS counters
               (owner_user_id, project_id, author_action_sequence,
                authoritative_commit_sequence, project_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 1, 1)
             ON CONFLICT (owner_user_id, project_id)
             DO UPDATE SET
               author_action_sequence = counters.author_action_sequence + 1,
               authoritative_commit_sequence = counters.authoritative_commit_sequence + 1,
               project_activity_position = counters.project_activity_position + 1
             RETURNING counters.author_action_sequence::text,
                       counters.authoritative_commit_sequence::text,
                       counters.project_activity_position::text",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?;
    let counter = |index: usize| row.get::<_, String>(index).parse().map_err(unavailable);
    Ok(CommitCounters {
        author_action_sequence: counter(/*index*/ 0)?,
        authoritative_commit_sequence: counter(/*index*/ 1)?,
        project_activity_position: counter(/*index*/ 2)?,
    })
}

/// Inserts the Author Action of one `authoritative_applied` outcome. A Current Chapter change has
/// no Authoritative Commit.
pub(crate) async fn insert_author_action(
    client: &Client,
    scope: &ProjectScope,
    author_action_sequence: u64,
    disposition: &ActionDisposition,
    authoritative_commit_id: Option<&str>,
    receipt_id: &str,
) -> Result<(), ProjectCommandError> {
    let (disposition, source_sequence) = match disposition {
        ActionDisposition::Forward => ("forward", None),
        ActionDisposition::Compensation { source_sequence } => {
            ("compensation", Some(source_sequence.to_string()))
        }
    };
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                compensated_source_sequence, authoritative_commit_id, receipt_id,
                receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4,
                     $5::text::numeric, $6::text::uuid, $7::text::uuid, 'authoritative_applied')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &disposition,
                &source_sequence,
                &authoritative_commit_id,
                &receipt_id,
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
                "SELECT tree_revision::text, current_chapter_id::text
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
    Ok(LockedProject {
        tree_revision: row
            .get::<_, String>(/*idx*/ 0)
            .parse()
            .map_err(unavailable)?,
        current_chapter_id: row.get(/*idx*/ 1),
    })
}
