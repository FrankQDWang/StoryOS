use std::fmt::Write as _;

use sha2::{Digest, Sha256};
use storyos_application::{
    ClaimedReadableExport, CompleteReadableExport, CompleteReadableExportError, ProjectId,
    ProjectReadError, ProjectScope, ReadableExportWorkStore, UserId,
    render_readable_manuscript_from_facts,
};
use storyos_core::{ProjectLifecycle, READABLE_EXPORT_PROFILE};
use uuid::Uuid;

use super::*;
use crate::manuscript_search::{LiveChapterReadExtent, read_live_chapter_blocks};
use crate::manuscript_tree::load_live_tree_facts;
use crate::snapshot::PinnedSnapshot;

impl ReadableExportWorkStore for PostgresProjectReader {
    async fn claim_next_readable_export(
        &self,
    ) -> Result<Option<ClaimedReadableExport>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        transaction
            .execute(
                "SELECT set_config('storyos.scope_mode', 'worker', true),
                        set_config('storyos.owner_user_id', '', true),
                        set_config('storyos.project_id', '', true),
                        set_config('storyos.user_id', '', true)",
                &[],
            )
            .await
            .map_err(read_error)?;
        let lease_seconds = i64::try_from(self.readable_export_lease_ttl.as_secs())
            .map_err(ProjectReadError::unavailable)?;
        let claimed = transaction
            .query_opt(
                "WITH next_work AS (
                   SELECT owner_user_id, project_id, export_id
                     FROM storyos.human_readable_manuscript_export_operations AS operation
                    WHERE operation.settled_result IS NULL
                      AND (
                        operation.wakeup_pending
                        OR (
                          operation.claim_generation > 0
                          AND operation.lease_expires_at IS NOT NULL
                          AND operation.lease_expires_at <= clock_timestamp()
                        )
                      )
                    ORDER BY operation.created_at
                    FOR UPDATE SKIP LOCKED
                    LIMIT 1
                 )
                 UPDATE storyos.human_readable_manuscript_export_operations AS operation
                    SET claim_generation = operation.claim_generation + 1,
                        fence_token = operation.claim_generation + 1,
                        lease_expires_at = clock_timestamp()
                          + ($1::bigint * interval '1 second'),
                        wakeup_pending = false
                   FROM next_work
                  WHERE operation.owner_user_id = next_work.owner_user_id
                    AND operation.project_id = next_work.project_id
                    AND operation.export_id = next_work.export_id
              RETURNING operation.owner_user_id::text,
                        operation.project_id::text,
                        operation.export_id::text,
                        operation.fence_token,
                        operation.source_snapshot_id::text,
                        operation.author_command_admission_id::text,
                        operation.command_id::text,
                        operation.idempotency_key::text",
                &[&lease_seconds],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        Ok(claimed.map(|row| ClaimedReadableExport {
            project_scope: ProjectScope::new(
                UserId::new(row.get::<_, String>(0)),
                ProjectId::new(row.get::<_, String>(1)),
            ),
            export_id: row.get(2),
            fence_token: row.get(3),
            source_snapshot_id: row.get(4),
            author_command_admission_id: row.get(5),
            command_id: row.get(6),
            idempotency_key: row.get(7),
        }))
    }

    async fn complete_readable_export(
        &self,
        claim: &ClaimedReadableExport,
    ) -> Result<CompleteReadableExport, CompleteReadableExportError> {
        let transaction = self
            .begin_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_challenge_error)?;
        let result = complete_claimed_export(&transaction.client, claim).await;
        match &result {
            Ok(_) => transaction
                .commit()
                .await
                .map_err(complete_challenge_error)?,
            Err(_) => {
                let _rollback = transaction.rollback().await;
            }
        }
        result
    }
}

async fn complete_claimed_export(
    client: &tokio_postgres::Client,
    claim: &ClaimedReadableExport,
) -> Result<CompleteReadableExport, CompleteReadableExportError> {
    let Some(operation) = client
        .query_opt(
            "SELECT operation.settled_result,
                    EXISTS (
                      SELECT 1 FROM storyos.human_readable_manuscript_exports AS output
                       WHERE output.owner_user_id = operation.owner_user_id
                         AND output.project_id = operation.project_id
                         AND output.export_id = operation.export_id
                    )
               FROM storyos.human_readable_manuscript_export_operations AS operation
              WHERE operation.owner_user_id = $1::text::uuid
                AND operation.project_id = $2::text::uuid
                AND operation.export_id = $3::text::uuid
                AND operation.fence_token = $4
              FOR UPDATE",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.export_id,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(complete_database_error)?
    else {
        return Err(CompleteReadableExportError::StaleFence);
    };
    if operation.get::<_, Option<String>>(0).is_some() || operation.get::<_, bool>(1) {
        return Ok(CompleteReadableExport::AlreadySettled);
    }
    let canonical_command_digest: String = client
        .query_one(
            "SELECT canonical_command_digest
               FROM storyos.author_command_admissions
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.author_command_admission_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get(0);
    let Some(project) = client
        .query_opt(
            "SELECT lifecycle_state FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(complete_database_error)?
    else {
        return persist_failed_export(client, claim, &canonical_command_digest, "archived_project")
            .await;
    };
    let lifecycle = match project.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(CompleteReadableExportError::unavailable(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            ));
        }
    };
    if lifecycle == ProjectLifecycle::Archived {
        return persist_failed_export(client, claim, &canonical_command_digest, "archived_project")
            .await;
    }
    match crate::snapshot::load_pinned_snapshot(
        client,
        &claim.project_scope,
        &claim.source_snapshot_id,
    )
    .await
    .map_err(complete_read_error)?
    {
        PinnedSnapshot::Available(snapshot) => {
            let tree = load_live_tree_facts(client, &claim.project_scope, snapshot.clone())
                .await
                .map_err(complete_read_error)?;
            let chapters = read_live_chapter_blocks(
                client,
                &claim.project_scope,
                LiveChapterReadExtent::AllChapters,
            )
            .await
            .map_err(complete_read_error)?;
            let manuscript_utf8 = render_readable_manuscript_from_facts(&tree, &chapters);
            persist_ready_export(
                client,
                claim,
                &canonical_command_digest,
                &snapshot,
                &manuscript_utf8,
            )
            .await
        }
        PinnedSnapshot::Expired | PinnedSnapshot::Missing => {
            persist_failed_export(client, claim, &canonical_command_digest, "archived_project")
                .await
        }
    }
}

async fn persist_ready_export(
    client: &tokio_postgres::Client,
    claim: &ClaimedReadableExport,
    canonical_command_digest: &str,
    snapshot: &storyos_application::CanonicalSnapshot,
    manuscript_utf8: &str,
) -> Result<CompleteReadableExport, CompleteReadableExportError> {
    let receipt_id = Uuid::now_v7().to_string();
    let content_sha256 = Sha256::digest(manuscript_utf8.as_bytes()).iter().fold(
        String::with_capacity(64),
        |mut value, byte| {
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        },
    );
    insert_export_receipt(
        client,
        claim,
        canonical_command_digest,
        &receipt_id,
        "authoritative_applied",
        "{}",
    )
    .await?;
    let project_activity_position = client
        .query_one(
            "INSERT INTO storyos.scope_counters AS counters
               (owner_user_id, project_id, project_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1)
             ON CONFLICT (owner_user_id, project_id)
             DO UPDATE SET
               project_activity_position = counters.project_activity_position + 1
             RETURNING counters.project_activity_position::text",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get::<_, String>(0);
    let project_activity_event_id = Uuid::now_v7().to_string();
    let payload = serde_json::json!({
        "kind": "human_readable_manuscript_export_settled",
        "export_id": claim.export_id,
        "content_sha256": content_sha256,
    })
    .to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     'human_readable_manuscript_export_settled', $5::text::uuid,
                     'authoritative_applied', $6::text::jsonb)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &project_activity_position,
                &project_activity_event_id,
                &receipt_id,
                &payload,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.human_readable_manuscript_exports
               (owner_user_id, project_id, export_id, receipt_id, source_snapshot_id,
                source_activity_position, export_profile, manuscript_utf8, content_sha256)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                     $6::text::bigint, $7, $8, $9)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.export_id,
                &receipt_id,
                &snapshot.snapshot_id,
                &snapshot.project_activity_position.to_string(),
                &READABLE_EXPORT_PROFILE,
                &manuscript_utf8,
                &content_sha256,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    settle_export_operation(client, claim, &receipt_id, Some("ready")).await?;
    Ok(CompleteReadableExport::SettledReady)
}

async fn persist_failed_export(
    client: &tokio_postgres::Client,
    claim: &ClaimedReadableExport,
    canonical_command_digest: &str,
    reason: &str,
) -> Result<CompleteReadableExport, CompleteReadableExportError> {
    let receipt_id = Uuid::now_v7().to_string();
    let result_payload = format!(r#"{{"reason":"{reason}"}}"#);
    insert_export_receipt(
        client,
        claim,
        canonical_command_digest,
        &receipt_id,
        "refused",
        &result_payload,
    )
    .await?;
    settle_export_operation(client, claim, &receipt_id, Some("failed")).await?;
    Ok(CompleteReadableExport::SettledFailed)
}

async fn insert_export_receipt(
    client: &tokio_postgres::Client,
    claim: &ClaimedReadableExport,
    canonical_command_digest: &str,
    receipt_id: &str,
    result_kind: &str,
    result_payload: &str,
) -> Result<(), CompleteReadableExportError> {
    client
        .execute(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'exportHumanReadableManuscript', $6, $7::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], $8, $9::text::jsonb)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &receipt_id,
                &claim.author_command_admission_id,
                &claim.command_id,
                &canonical_command_digest,
                &claim.idempotency_key,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.author_command_admission_id,
                &receipt_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

async fn settle_export_operation(
    client: &tokio_postgres::Client,
    claim: &ClaimedReadableExport,
    receipt_id: &str,
    settled_result: Option<&str>,
) -> Result<(), CompleteReadableExportError> {
    let updated = client
        .execute(
            "UPDATE storyos.human_readable_manuscript_export_operations
                SET settled_result = $5,
                    wakeup_pending = false
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND export_id = $3::text::uuid
                AND fence_token = $4",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.export_id,
                &claim.fence_token,
                &settled_result,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    if updated != 1 {
        return Err(CompleteReadableExportError::StaleFence);
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'exportHumanReadableManuscript'
                AND idempotency_key = $4::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &receipt_id,
                &claim.idempotency_key,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

fn complete_challenge_error(
    error: storyos_application::ProjectCommandChallengeError,
) -> CompleteReadableExportError {
    CompleteReadableExportError::unavailable(error)
}

fn complete_database_error(error: tokio_postgres::Error) -> CompleteReadableExportError {
    CompleteReadableExportError::unavailable(error)
}

fn complete_read_error(error: ProjectReadError) -> CompleteReadableExportError {
    CompleteReadableExportError::unavailable(error)
}
