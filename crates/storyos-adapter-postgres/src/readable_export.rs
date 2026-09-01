use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, CanonicalSnapshot, CanonicalTreeFacts, ChapterFact,
    ExportHumanReadableManuscriptCommand, ExportHumanReadableManuscriptError,
    ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptSettlementEffect,
    ExportHumanReadableManuscriptStore, GetHumanReadableManuscriptExport,
    HumanReadableManuscriptExportPage, HumanReadableManuscriptExportReader,
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ProjectReadError, ProjectScope,
    VolumeFact, VolumeId, render_readable_manuscript_from_facts,
};
use storyos_core::{
    ExportHumanReadableManuscript as CoreExport, ExportHumanReadableManuscriptResult,
    ProjectLifecycle, ProjectPresence, READABLE_EXPORT_PROFILE,
    export_human_readable_manuscript as classify_export,
};
use uuid::Uuid;

use super::*;
use crate::manuscript_search::read_live_chapter_blocks;

impl ExportHumanReadableManuscriptStore for PostgresProjectReader {
    async fn export_human_readable_manuscript(
        &self,
        command: &ExportHumanReadableManuscriptCommand,
    ) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(export_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(export_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(export_challenge_error)?;
                read_export_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(export_challenge_error)?;
                Err(ExportHumanReadableManuscriptError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_export(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(export_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        let _rollback = transaction.rollback().await;
                        Err(error)
                    }
                }
            }
        }
    }
}

impl HumanReadableManuscriptExportReader for PostgresProjectReader {
    async fn read_human_readable_manuscript_export(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<GetHumanReadableManuscriptExport, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let Some(project) = transaction
            .query_opt(
                "SELECT lifecycle_state FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Missing);
        };
        if project.get::<_, String>(0) == "archived" {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Archived);
        }
        let Some(row) = transaction
            .query_opt(
                "SELECT export.export_id::text,
                        export.export_profile,
                        export.content_sha256,
                        export.manuscript_utf8,
                        snapshot.snapshot_id::text,
                        snapshot.project_activity_position::text,
                        snapshot.replay_generation::text,
                        snapshot.redaction_profile,
                        snapshot.schema_profile,
                        to_char(snapshot.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        CASE WHEN snapshot.expires_at IS NULL THEN NULL
                             ELSE to_char(snapshot.expires_at AT TIME ZONE 'UTC',
                                          'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                        END,
                        current_floor.floor_position::text,
                        snapshot.expires_at IS NOT NULL
                          AND snapshot.expires_at <= clock_timestamp()
                   FROM storyos.human_readable_manuscript_exports AS export
                   JOIN storyos.project_snapshots AS snapshot
                     ON snapshot.owner_user_id = export.owner_user_id
                    AND snapshot.project_id = export.project_id
                    AND snapshot.snapshot_id::text = export.source_snapshot_id
                   JOIN LATERAL (
                     SELECT replay_generation FROM storyos.replay_generations
                      WHERE owner_user_id = snapshot.owner_user_id
                        AND project_id = snapshot.project_id
                      ORDER BY replay_generation DESC LIMIT 1
                   ) AS current_generation ON true
                   JOIN storyos.replay_floors AS current_floor
                     ON (current_floor.owner_user_id, current_floor.project_id,
                         current_floor.replay_generation) =
                        (snapshot.owner_user_id, snapshot.project_id,
                         current_generation.replay_generation)
                  WHERE export.owner_user_id = $1::text::uuid
                    AND export.project_id = $2::text::uuid
                    AND export.export_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &export_id,
                ],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Missing);
        };
        if row.get::<_, bool>(12) {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Expired);
        }
        transaction.commit().await.map_err(read_error)?;
        Ok(GetHumanReadableManuscriptExport::Ready(Box::new(
            HumanReadableManuscriptExportPage {
                project_scope: scope.clone(),
                export_id: row.get(0),
                export_profile: row.get(1),
                content_sha256: row.get(2),
                manuscript_utf8: row.get(3),
                source_snapshot: CanonicalSnapshot {
                    snapshot_id: row.get(4),
                    project_activity_position: row
                        .get::<_, String>(5)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                    replay_generation: row
                        .get::<_, String>(6)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                    redaction_profile: row.get(7),
                    schema_profile: row.get(8),
                    created_at: row.get(9),
                    expires_at: row.get(10),
                    floor_position: row
                        .get::<_, String>(11)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                },
            },
        )))
    }
}

async fn persist_export(
    client: &tokio_postgres::Client,
    command: &ExportHumanReadableManuscriptCommand,
) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(export_database_error)?;
    let Some(row) = row else {
        return Err(ExportHumanReadableManuscriptError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(ExportHumanReadableManuscriptError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let classified = classify_export(&CoreExport {
        presence: ProjectPresence::Present,
        current_lifecycle,
    });
    let effect = match classified {
        ExportHumanReadableManuscriptResult::Applied => {
            let snapshot =
                crate::snapshot::load_latest_canonical_snapshot(client, &command.project_scope)
                    .await
                    .map_err(export_read_error)?
                    .ok_or_else(|| {
                        ExportHumanReadableManuscriptError::Unavailable(Box::new(
                            std::io::Error::other(
                                "canonical snapshot is required for human-readable export",
                            ),
                        ))
                    })?;
            let tree = load_tree_facts(client, &command.project_scope, snapshot.clone()).await?;
            let chapters = read_live_chapter_blocks(client, &command.project_scope)
                .await
                .map_err(export_read_error)?;
            let manuscript_utf8 = render_readable_manuscript_from_facts(&tree, &chapters);
            let content_sha256 = Sha256::digest(manuscript_utf8.as_bytes()).iter().fold(
                String::with_capacity(64),
                |mut value, byte| {
                    use std::fmt::Write as _;
                    write!(value, "{byte:02x}").expect("writing to String cannot fail");
                    value
                },
            );
            ExportHumanReadableManuscriptSettlementEffect::Applied {
                content_sha256,
                manuscript_utf8,
                source_snapshot: snapshot,
            }
        }
        ExportHumanReadableManuscriptResult::Refused {
            reason: storyos_core::ExportHumanReadableManuscriptRefusal::MissingProject,
        } => return Err(ExportHumanReadableManuscriptError::MissingProject),
        ExportHumanReadableManuscriptResult::Refused { reason } => {
            ExportHumanReadableManuscriptSettlementEffect::Refused { reason }
        }
    };
    insert_export_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        ExportHumanReadableManuscriptSettlementEffect::Applied { .. } => {
            ("authoritative_applied", "{}".to_owned())
        }
        ExportHumanReadableManuscriptSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::ExportHumanReadableManuscriptRefusal::ArchivedProject => {
                    "archived_project"
                }
                storyos_core::ExportHumanReadableManuscriptRefusal::MissingProject => {
                    return Err(ExportHumanReadableManuscriptError::MissingProject);
                }
            };
            ("refused", format!(r#"{{"reason":"{refused}"}}"#))
        }
    };
    let receipt_created_at = client
        .query_one(
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
                     '{}'::text[], $8, $9::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(export_database_error)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(export_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let ExportHumanReadableManuscriptSettlementEffect::Applied {
        content_sha256,
        manuscript_utf8,
        source_snapshot,
    } = &effect
    {
        project_activity_position = client
            .query_one(
                "INSERT INTO storyos.scope_counters AS counters
                   (owner_user_id, project_id, project_activity_position)
                 VALUES ($1::text::uuid, $2::text::uuid, 1)
                 ON CONFLICT (owner_user_id, project_id)
                 DO UPDATE SET
                   project_activity_position = counters.project_activity_position + 1
                 RETURNING counters.project_activity_position::text",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(export_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(export_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "human_readable_manuscript_export_settled",
            "export_id": command.export_id,
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
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &project_activity_position.to_string(),
                    &project_activity_event_id,
                    &command.ids.receipt_id,
                    &payload,
                ],
            )
            .await
            .map_err(export_database_error)?;
        client
            .execute(
                "INSERT INTO storyos.human_readable_manuscript_exports
                   (owner_user_id, project_id, export_id, receipt_id, source_snapshot_id,
                    source_activity_position, export_profile, manuscript_utf8, content_sha256)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                         $6::text::bigint, $7, $8, $9)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.export_id,
                    &command.ids.receipt_id,
                    &source_snapshot.snapshot_id,
                    &source_snapshot.project_activity_position.to_string(),
                    &READABLE_EXPORT_PROFILE,
                    manuscript_utf8,
                    content_sha256,
                ],
            )
            .await
            .map_err(export_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'exportHumanReadableManuscript' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(export_database_error)?;
    Ok(ExportHumanReadableManuscriptSettlement {
        ids: command.ids.clone(),
        export_id: command.export_id.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn load_tree_facts(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    snapshot: CanonicalSnapshot,
) -> Result<CanonicalTreeFacts, ExportHumanReadableManuscriptError> {
    let tree_revision = client
        .query_one(
            "SELECT tree_revision::text FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(export_database_error)?
        .get::<_, String>(0)
        .parse::<u64>()
        .map_err(export_parse_error)?;
    let volume_rows = client
        .query(
            "SELECT manuscript_object_id::text, title
               FROM storyos.manuscript_objects AS volume
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND object_kind = 'volume'
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.volume_removal_decisions AS removal
                   WHERE removal.owner_user_id = volume.owner_user_id
                     AND removal.project_id = volume.project_id
                     AND removal.volume_id = volume.manuscript_object_id
                )
              ORDER BY tree_order",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(export_database_error)?;
    let chapter_rows = client
        .query(
            "SELECT parent_volume_id::text, manuscript_object_id::text, title
               FROM storyos.manuscript_objects AS chapter
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND object_kind = 'chapter'
                AND parent_volume_id IS NOT NULL
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                   WHERE removal.owner_user_id = chapter.owner_user_id
                     AND removal.project_id = chapter.project_id
                     AND removal.chapter_id = chapter.manuscript_object_id
                )
              ORDER BY parent_volume_id, tree_order",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(export_database_error)?;
    let mut chapters_by_volume = std::collections::BTreeMap::<String, Vec<ChapterFact>>::new();
    for chapter in chapter_rows {
        let volume_id = chapter.get::<_, String>(0);
        let chapters = chapters_by_volume.entry(volume_id).or_default();
        let next_order = chapters.len() as u64 + 1;
        chapters.push(ChapterFact {
            project_scope: scope.clone(),
            chapter_id: storyos_application::ChapterId::new(chapter.get::<_, String>(1)),
            title: chapter.get(2),
            order: next_order,
        });
    }
    let mut volumes = Vec::with_capacity(volume_rows.len());
    for volume in volume_rows {
        let volume_id = volume.get::<_, String>(0);
        let next_order = volumes.len() as u64 + 1;
        volumes.push(VolumeFact {
            project_scope: scope.clone(),
            chapters: chapters_by_volume.remove(&volume_id).unwrap_or_default(),
            volume_id: VolumeId::new(volume_id),
            title: volume.get(1),
            order: next_order,
        });
    }
    Ok(CanonicalTreeFacts {
        project_scope: scope.clone(),
        snapshot,
        tree_revision,
        volumes,
    })
}

async fn insert_export_admission(
    client: &tokio_postgres::Client,
    command: &ExportHumanReadableManuscriptCommand,
) -> Result<(), ExportHumanReadableManuscriptError> {
    let client_session_generation = command.client_binding.session_generation.to_string();
    let inserted = client
        .execute(
            "INSERT INTO storyos.author_command_admissions
               (owner_user_id, project_id, author_command_admission_id, command_id,
                editor_session_id, writer_generation, client_session_binding_ref,
                client_session_generation, client_contract_revision, security_policy_revision,
                action_class, method, route_template, command_schema, command_kind,
                canonical_command_digest, idempotency_key, challenge_consumed_at,
                challenge_expires_at, correlation_id, chapter_object_id,
                expected_authoritative_revision_id, expected_proposal_head_revision_ids,
                target_refs, observed_ownership_partition, editor_contract_revision,
                undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    NULL, NULL, $5, $6::text::numeric, $7, $8,
                    'explicit_project_command', $9, $10, $11, 'exportHumanReadableManuscript',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'exportHumanReadableManuscript'
                AND challenge.idempotency_key = $13::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.client_binding.binding_ref,
                &client_session_generation,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
                &command.challenge_binding.method,
                &command.challenge_binding.route_template,
                &command.challenge_binding.command_schema,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &command.correlation_id,
                &command.canonical_command_bytes.as_slice(),
            ],
        )
        .await
        .map_err(export_database_error)?;
    if inserted != 1 {
        return Err(ExportHumanReadableManuscriptError::InvalidChallenge);
    }
    Ok(())
}

async fn read_export_settlement(
    store: &PostgresProjectReader,
    command: &ExportHumanReadableManuscriptCommand,
    receipt_id: &str,
) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(export_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(export_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(export_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'export_id',
                        payload.payload->>'content_sha256',
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text,
                        export.manuscript_utf8,
                        export.source_snapshot_id
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key,
                         idempotency.result_reference) =
                        (receipt.owner_user_id, receipt.project_id, receipt.command_kind,
                         receipt.idempotency_key, receipt.receipt_id::text)
              LEFT JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.human_readable_manuscript_exports AS export
                     ON (export.owner_user_id, export.project_id, export.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'exportHumanReadableManuscript'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(export_database_error)?
            .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                ExportHumanReadableManuscriptSettlementEffect::Applied {
                    content_sha256: row
                        .get::<_, Option<String>>(7)
                        .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?,
                    manuscript_utf8: row
                        .get::<_, Option<String>>(10)
                        .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?,
                    source_snapshot: CanonicalSnapshot {
                        snapshot_id: row
                            .get::<_, Option<String>>(11)
                            .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?,
                        project_activity_position: 0,
                        replay_generation: 1,
                        floor_position: 0,
                        redaction_profile: "storyos.author.v1".to_owned(),
                        schema_profile: "storyos.public.release.1".to_owned(),
                        created_at: row.get(3),
                        expires_at: None,
                    },
                }
            }
            ("refused", Some("archived_project")) => {
                ExportHumanReadableManuscriptSettlementEffect::Refused {
                    reason: storyos_core::ExportHumanReadableManuscriptRefusal::ArchivedProject,
                }
            }
            _ => return Err(ExportHumanReadableManuscriptError::BindingConflict),
        };
        let export_id = row.get::<_, Option<String>>(6).unwrap_or_default();
        Ok(ExportHumanReadableManuscriptSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            export_id: if export_id.is_empty() {
                command.export_id.clone()
            } else {
                export_id
            },
            receipt_created_at: row.get(3),
            effect,
            project_activity_position: row
                .get::<_, Option<String>>(8)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(export_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(9).unwrap_or_default(),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(export_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn export_challenge_error(
    error: ProjectCommandChallengeError,
) -> ExportHumanReadableManuscriptError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            ExportHumanReadableManuscriptError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired => {
            ExportHumanReadableManuscriptError::InvalidChallenge
        }
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
        }
    }
}

fn export_database_error(error: tokio_postgres::Error) -> ExportHumanReadableManuscriptError {
    ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
}

fn export_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ExportHumanReadableManuscriptError {
    ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
}

fn export_read_error(error: ProjectReadError) -> ExportHumanReadableManuscriptError {
    ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
}
