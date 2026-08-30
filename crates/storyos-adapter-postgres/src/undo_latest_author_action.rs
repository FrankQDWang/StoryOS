use storyos_application::{
    AuthorCommandAdmissionIds, EditorSessionError, ProjectCommandChallengeError,
    ProjectCommandChallengeUse, UndoLatestAuthorActionCommand, UndoLatestAuthorActionError,
    UndoLatestAuthorActionSettlement, UndoLatestAuthorActionSettlementEffect,
    UndoLatestAuthorActionStore,
};
use storyos_core::{
    AuthorUndoFrontier, AuthorUndoFrontierKind, UndoLatestAuthorAction as CoreUndo,
    UndoLatestAuthorActionConflict, UndoLatestAuthorActionResult,
    undo_latest_author_action as classify_undo,
};
use uuid::Uuid;

use super::*;
use crate::author_edit::{parse_u64, sha256_hex};

impl UndoLatestAuthorActionStore for PostgresProjectReader {
    async fn undo_latest_author_action(
        &self,
        command: &UndoLatestAuthorActionCommand,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(undo_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(undo_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction.rollback().await.map_err(undo_challenge_error)?;
                read_undo_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction.rollback().await.map_err(undo_challenge_error)?;
                Err(UndoLatestAuthorActionError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_undo(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(undo_challenge_error)?;
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

struct ObservedFrontier {
    sequence: u64,
    chapter_id: String,
    resulting_revision_id: String,
    prior_revision_id: String,
    prior_payload: String,
    current_head_revision_id: String,
}

async fn persist_undo(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let row = client
        .query_opt(
            "SELECT project.lifecycle_state,
                    frontier.author_action_sequence::text,
                    frontier.manuscript_object_id::text,
                    frontier.resulting_revision_id::text,
                    frontier.prior_revision_id::text,
                    convert_from(prior_payload.canonical_bytes, 'UTF8'),
                    head.current_revision_id::text
               FROM storyos.projects AS project
          LEFT JOIN LATERAL (
                SELECT action.author_action_sequence,
                       commit.manuscript_object_id,
                       commit.resulting_revision_id,
                       commit.prior_revision_id
                  FROM storyos.author_action_entries AS action
                  JOIN storyos.authoritative_commits AS commit
                    ON (commit.owner_user_id, commit.project_id, commit.receipt_id,
                        commit.receipt_result_kind, commit.authoritative_commit_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id,
                        action.receipt_result_kind, action.authoritative_commit_id)
             LEFT JOIN storyos.author_action_entries AS compensation
                    ON compensation.owner_user_id = action.owner_user_id
                   AND compensation.project_id = action.project_id
                   AND compensation.disposition = 'compensation'
                   AND compensation.compensated_source_sequence = action.author_action_sequence
                 WHERE action.owner_user_id = project.owner_user_id
                   AND action.project_id = project.project_id
                   AND action.disposition = 'forward'
                   AND compensation.author_action_sequence IS NULL
              ORDER BY action.author_action_sequence DESC
                 LIMIT 1
          ) AS frontier ON true
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (project.owner_user_id, project.project_id, frontier.manuscript_object_id)
          LEFT JOIN storyos.authoritative_revisions AS prior_revision
                 ON (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.manuscript_object_id, prior_revision.revision_id) =
                    (project.owner_user_id, project.project_id,
                     frontier.manuscript_object_id, frontier.prior_revision_id)
          LEFT JOIN storyos.authoritative_payloads AS prior_payload
                 ON (prior_payload.owner_user_id, prior_payload.project_id,
                     prior_payload.payload_id) =
                    (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.payload_id)
              WHERE project.owner_user_id = $1::text::uuid
                AND project.project_id = $2::text::uuid
              FOR UPDATE OF project",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let Some(row) = row else {
        return Err(UndoLatestAuthorActionError::MissingProject);
    };
    if row.get::<_, String>(0) != "active" {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    let observed = match (
        row.get::<_, Option<String>>(1),
        row.get::<_, Option<String>>(2),
        row.get::<_, Option<String>>(3),
        row.get::<_, Option<String>>(4),
        row.get::<_, Option<String>>(5),
        row.get::<_, Option<String>>(6),
    ) {
        (
            Some(sequence),
            Some(chapter_id),
            Some(resulting_revision_id),
            Some(prior_revision_id),
            Some(prior_payload),
            Some(current_head_revision_id),
        ) => Some(ObservedFrontier {
            sequence: sequence.parse().map_err(undo_parse_error)?,
            chapter_id,
            resulting_revision_id,
            prior_revision_id,
            prior_payload,
            current_head_revision_id,
        }),
        _ => None,
    };
    let classified = classify_undo(&CoreUndo {
        expected_author_undo_frontier_sequence: command.expected_author_undo_frontier_sequence,
        current_author_undo_frontier: observed.as_ref().map(|frontier| AuthorUndoFrontier {
            sequence: frontier.sequence,
            kind: AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
                resulting_revision_id: frontier.resulting_revision_id.clone(),
            },
        }),
        expected_head_revision_id: command.expected_authoritative_revision_id.clone(),
        current_head_revision_id: observed
            .as_ref()
            .map(|frontier| frontier.current_head_revision_id.clone())
            .unwrap_or_default(),
    });
    let admission_chapter = observed
        .as_ref()
        .map(|frontier| frontier.chapter_id.as_str());
    let admission_expected = observed
        .as_ref()
        .map(|frontier| frontier.resulting_revision_id.as_str());
    insert_undo_admission(client, command, admission_chapter, admission_expected).await?;
    match classified {
        UndoLatestAuthorActionResult::Compensated { source_sequence } => {
            let Some(frontier) = observed.as_ref() else {
                return Err(UndoLatestAuthorActionError::BindingConflict);
            };
            persist_compensation(client, command, frontier, source_sequence).await
        }
        UndoLatestAuthorActionResult::Conflicted { reason } => {
            persist_zero_authority(
                client,
                command,
                observed.as_ref(),
                (
                    "conflicted",
                    match &reason {
                        UndoLatestAuthorActionConflict::FrontierMismatch { .. } => {
                            "frontier_mismatch"
                        }
                        UndoLatestAuthorActionConflict::WrongTargetHead => "wrong_target_head",
                    },
                    UndoLatestAuthorActionSettlementEffect::Conflicted { reason },
                ),
            )
            .await
        }
        UndoLatestAuthorActionResult::Unavailable { reason } => {
            persist_zero_authority(
                client,
                command,
                observed.as_ref(),
                (
                    "refused",
                    match reason {
                        storyos_core::UndoLatestAuthorActionUnavailable::NoFrontier => {
                            "no_frontier"
                        }
                        storyos_core::UndoLatestAuthorActionUnavailable::Barrier => "barrier",
                    },
                    UndoLatestAuthorActionSettlementEffect::Unavailable { reason },
                ),
            )
            .await
        }
    }
}

async fn persist_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedFrontier,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let counter_row = client
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
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let author_action_sequence = parse_u64(counter_row.get(0)).map_err(undo_from_author_edit)?;
    let authoritative_commit_sequence =
        parse_u64(counter_row.get(1)).map_err(undo_from_author_edit)?;
    let project_activity_position = parse_u64(counter_row.get(2)).map_err(undo_from_author_edit)?;
    let revision_id = Uuid::now_v7().to_string();
    let payload_id = Uuid::now_v7().to_string();
    let authoritative_commit_id = Uuid::now_v7().to_string();
    let project_activity_event_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8'))",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &payload_id,
                &frontier.prior_payload,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &frontier.chapter_id,
                &revision_id,
                &payload_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let copied = crate::manuscript_block::copy_or_upgrade_revision_members(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &frontier.chapter_id,
        &frontier.prior_revision_id,
        &revision_id,
    )
    .await
    .map_err(undo_database_error)?;
    if copied == 0 {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("compensating revision members were not copied"),
        )));
    }
    let blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &frontier.chapter_id,
        &revision_id,
        &frontier.prior_payload,
    )
    .await
    .map_err(undo_database_error)?;
    let body = crate::manuscript_block::display_body_from_stored(&frontier.prior_payload, &blocks);
    client
        .execute(
            "INSERT INTO storyos.authoritative_revision_envelopes
               (owner_user_id, project_id, manuscript_object_id, revision_id, parent_revision_id,
                schema_revision, creator_kind, creator_ref, receipt_id, receipt_result_kind,
                cause_kind, payload_digest)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'storyos.authoritative-revision-envelope.v1',
                     'author_command_admission', $6::text::uuid, $7::text::uuid,
                     'authoritative_applied', 'direct_author_action', $8)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &frontier.chapter_id,
                &revision_id,
                &frontier.current_head_revision_id,
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
                &sha256_hex(frontier.prior_payload.as_bytes()),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.authoritative_heads SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &frontier.chapter_id,
                &revision_id,
                &frontier.current_head_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if head_updates != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    client
        .execute(
            "INSERT INTO storyos.authoritative_commits
               (owner_user_id, project_id, authoritative_commit_id, authoritative_commit_sequence,
                manuscript_object_id, prior_revision_id, resulting_revision_id,
                author_command_admission_id, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, 'authoritative_applied')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &authoritative_commit_id,
                &authoritative_commit_sequence.to_string(),
                &frontier.chapter_id,
                &frontier.current_head_revision_id,
                &revision_id,
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        "{}",
        &frontier.current_head_revision_id,
        &revision_id,
        Some((revision_id.clone(), authoritative_commit_id.clone())),
    )
    .await?;
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                compensated_source_sequence, authoritative_commit_id, receipt_id,
                receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'compensation',
                     $4::text::numeric, $5::text::uuid, $6::text::uuid, 'authoritative_applied')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &source_sequence.to_string(),
                &authoritative_commit_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.project_activity_events
               (owner_user_id, project_id, project_activity_position,
                project_activity_event_id, event_kind, receipt_id,
                receipt_result_kind, authoritative_commit_id,
                resulting_revision_id, author_action_sequence)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric,
                     $4::text::uuid, 'authoritative_author_edit_applied',
                     $5::text::uuid, 'authoritative_applied', $6::text::uuid,
                     $7::text::uuid, $8::text::numeric)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &project_activity_position.to_string(),
                &project_activity_event_id,
                &command.ids.receipt_id,
                &authoritative_commit_id,
                &revision_id,
                &author_action_sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let base_snapshot_id = Uuid::now_v7().to_string();
    let base_updates = client
        .execute(
            "UPDATE storyos.editor_session_base_snapshots AS snapshot
                SET snapshot_id = $4::text::uuid,
                    authoritative_revision_id = $5::text::uuid,
                    project_activity_position = $6::text::numeric,
                    created_at = clock_timestamp()
               FROM storyos.project_writer_generations AS writer
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid
                AND snapshot.authoritative_revision_id = $7::text::uuid
                AND (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (snapshot.owner_user_id, snapshot.project_id,
                     snapshot.editor_session_id)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.editor_session_id.as_ref(),
                &base_snapshot_id,
                &revision_id,
                &project_activity_position.to_string(),
                &frontier.current_head_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if base_updates != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    crate::snapshot::persist_canonical_snapshot(
        client,
        &command.project_scope,
        &base_snapshot_id,
        project_activity_position,
    )
    .await
    .map_err(undo_database_error)?;
    settle_idempotency(client, command).await?;
    let author_undo_frontier_sequence =
        crate::editor_session::current_author_undo_frontier_sequence(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
        )
        .await
        .map_err(undo_from_session)?;
    Ok(UndoLatestAuthorActionSettlement {
        ids: command.ids.clone(),
        effect: UndoLatestAuthorActionSettlementEffect::Compensated {
            source_sequence,
            author_action_sequence,
            authoritative_commit_id,
            revision_id,
            body,
            blocks,
            author_undo_frontier_sequence,
        },
        receipt_created_at,
        project_activity_position,
    })
}

async fn persist_zero_authority(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: Option<&ObservedFrontier>,
    classified: (
        &'static str,
        &'static str,
        UndoLatestAuthorActionSettlementEffect,
    ),
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let (result_kind, payload_reason, effect) = classified;
    let head = frontier
        .map(|frontier| frontier.current_head_revision_id.as_str())
        .unwrap_or(command.expected_authoritative_revision_id.as_str());
    let result_payload = format!(r#"{{"reason":"{payload_reason}"}}"#);
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        result_kind,
        &result_payload,
        head,
        head,
        /*authority*/ None,
    )
    .await?;
    settle_idempotency(client, command).await?;
    Ok(UndoLatestAuthorActionSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position: 0,
    })
}

async fn insert_undo_admission(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    chapter_object_id: Option<&str>,
    expected_authoritative_revision_id: Option<&str>,
) -> Result<(), UndoLatestAuthorActionError> {
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
                    session.editor_session_id, writer.writer_generation,
                    session.client_session_binding_ref, $6::text::numeric,
                    session.client_contract_revision, session.security_policy_revision,
                    'explicit_editor_command', $7, $8, $9, 'undoLatestAuthorAction',
                    $10, $11::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $12::text::uuid, $13::text::uuid, $14::text::uuid, '{}'::uuid[], '{}'::text[],
                    NULL, session.client_contract_revision, NULL, NULL, NULL,
                    convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.editor_sessions AS session
               JOIN storyos.project_writer_generations AS writer
                 ON (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (session.owner_user_id, session.project_id, session.editor_session_id)
                AND writer.writer_generation = (
                  SELECT max(current_writer.writer_generation)
                    FROM storyos.project_writer_generations AS current_writer
                   WHERE current_writer.owner_user_id = session.owner_user_id
                     AND current_writer.project_id = session.project_id
                )
               JOIN storyos.project_command_challenges AS challenge
                 ON (challenge.owner_user_id, challenge.project_id,
                     challenge.command_kind, challenge.idempotency_key) =
                    (session.owner_user_id, session.project_id,
                     'undoLatestAuthorAction', $11::text::uuid)
              WHERE session.owner_user_id = $1::text::uuid
                AND session.project_id = $2::text::uuid
                AND session.editor_session_id = $5::text::uuid
                AND session.client_session_binding_ref = $16
                AND session.client_session_generation = $6::text::numeric
                AND session.client_contract_revision = $17
                AND session.security_policy_revision = $18
                AND challenge.consumed_at IS NOT NULL",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.editor_session_id.as_ref(),
                &client_session_generation,
                &command.challenge_binding.method,
                &command.challenge_binding.route_template,
                &command.challenge_binding.command_schema,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &command.correlation_id,
                &chapter_object_id,
                &expected_authoritative_revision_id,
                &command.canonical_command_bytes.as_slice(),
                &command.client_binding.binding_ref,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if inserted != 1 {
        return Err(UndoLatestAuthorActionError::InvalidChallenge);
    }
    Ok(())
}

async fn insert_undo_receipt(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    result_kind: &str,
    result_payload: &str,
    prior_head: &str,
    resulting_head: &str,
    authority: Option<(String, String)>,
) -> Result<String, UndoLatestAuthorActionError> {
    let (revision_ids, commit_ids) = match &authority {
        Some((revision_id, commit_id)) => (vec![revision_id.clone()], vec![commit_id.clone()]),
        None => (Vec::new(), Vec::new()),
    };
    let created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'undoLatestAuthorAction', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$9::text::uuid],
                     ARRAY[$10::text::uuid], $11::text[]::uuid[], '{}'::uuid[],
                     $12::text[]::uuid[], '{}'::text[], '{}'::text[], '{}'::text[],
                     $13, $14::text::jsonb)
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
                &command.expected_authoritative_revision_id,
                &prior_head,
                &resulting_head,
                &revision_ids,
                &commit_ids,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(undo_database_error)?
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
        .map_err(undo_database_error)?;
    Ok(created_at)
}

async fn settle_idempotency(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<(), UndoLatestAuthorActionError> {
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'undoLatestAuthorAction'
                AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    Ok(())
}

async fn read_undo_settlement(
    store: &PostgresProjectReader,
    command: &UndoLatestAuthorActionCommand,
    receipt_id: &str,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(undo_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(undo_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(undo_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        action.compensated_source_sequence::text,
                        action.author_action_sequence::text,
                        commit.authoritative_commit_id::text,
                        commit.resulting_revision_id::text,
                        convert_from(payload.canonical_bytes, 'UTF8'),
                        activity.project_activity_position::text,
                        commit.manuscript_object_id::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
              LEFT JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id, action.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.authoritative_commits AS commit
                     ON (commit.owner_user_id, commit.project_id, commit.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.authoritative_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id,
                         revision.manuscript_object_id, revision.revision_id) =
                        (commit.owner_user_id, commit.project_id,
                         commit.manuscript_object_id, commit.resulting_revision_id)
              LEFT JOIN storyos.authoritative_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                        (revision.owner_user_id, revision.project_id, revision.payload_id)
              LEFT JOIN storyos.project_activity_events AS activity
                     ON (activity.owner_user_id, activity.project_id, activity.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'undoLatestAuthorAction'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(undo_database_error)?
            .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
        let current_frontier = crate::editor_session::current_author_undo_frontier_sequence(
            &client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
        )
        .await
        .map_err(undo_from_session)?;
        let result_kind = row.get::<_, String>(3);
        let reason = row.get::<_, Option<String>>(4);
        let receipt_created_at = row.get::<_, String>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                let revision_id = row
                    .get::<_, Option<String>>(9)
                    .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
                let stored = row
                    .get::<_, Option<String>>(10)
                    .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
                let chapter_id = row
                    .get::<_, Option<String>>(12)
                    .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
                let blocks = crate::manuscript_block::load_revision_blocks(
                    &client,
                    command.project_scope.owner_user_id.as_ref(),
                    command.project_scope.project_id.as_ref(),
                    &chapter_id,
                    &revision_id,
                    &stored,
                )
                .await
                .map_err(undo_database_error)?;
                UndoLatestAuthorActionSettlementEffect::Compensated {
                    source_sequence: row
                        .get::<_, Option<String>>(6)
                        .ok_or(UndoLatestAuthorActionError::BindingConflict)?
                        .parse()
                        .map_err(undo_parse_error)?,
                    author_action_sequence: row
                        .get::<_, Option<String>>(7)
                        .ok_or(UndoLatestAuthorActionError::BindingConflict)?
                        .parse()
                        .map_err(undo_parse_error)?,
                    authoritative_commit_id: row
                        .get::<_, Option<String>>(8)
                        .ok_or(UndoLatestAuthorActionError::BindingConflict)?,
                    revision_id: revision_id.clone(),
                    body: crate::manuscript_block::display_body_from_stored(&stored, &blocks),
                    blocks,
                    author_undo_frontier_sequence: current_frontier,
                }
            }
            ("conflicted", Some("frontier_mismatch")) => {
                UndoLatestAuthorActionSettlementEffect::Conflicted {
                    reason: UndoLatestAuthorActionConflict::FrontierMismatch {
                        current_author_undo_frontier_sequence: current_frontier,
                    },
                }
            }
            ("conflicted", Some("wrong_target_head")) => {
                UndoLatestAuthorActionSettlementEffect::Conflicted {
                    reason: UndoLatestAuthorActionConflict::WrongTargetHead,
                }
            }
            ("refused", Some("no_frontier")) => {
                UndoLatestAuthorActionSettlementEffect::Unavailable {
                    reason: storyos_core::UndoLatestAuthorActionUnavailable::NoFrontier,
                }
            }
            ("refused", Some("barrier")) => UndoLatestAuthorActionSettlementEffect::Unavailable {
                reason: storyos_core::UndoLatestAuthorActionUnavailable::Barrier,
            },
            _ => return Err(UndoLatestAuthorActionError::BindingConflict),
        };
        let project_activity_position = row
            .get::<_, Option<String>>(11)
            .map(|value| value.parse::<u64>())
            .transpose()
            .map_err(undo_parse_error)?
            .unwrap_or(0);
        Ok(UndoLatestAuthorActionSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            effect,
            receipt_created_at,
            project_activity_position,
        })
    }
    .await;
    let _ = client.batch_execute("ROLLBACK").await;
    result
}

fn undo_challenge_error(error: ProjectCommandChallengeError) -> UndoLatestAuthorActionError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            UndoLatestAuthorActionError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            UndoLatestAuthorActionError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            UndoLatestAuthorActionError::Unavailable(source)
        }
    }
}

fn undo_database_error(error: tokio_postgres::Error) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn undo_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn undo_from_author_edit(
    error: storyos_application::AuthorEditError,
) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn undo_from_session(error: EditorSessionError) -> UndoLatestAuthorActionError {
    match error {
        EditorSessionError::BindingConflict | EditorSessionError::InvalidChallenge => {
            UndoLatestAuthorActionError::BindingConflict
        }
        EditorSessionError::Unavailable(source) => UndoLatestAuthorActionError::Unavailable(source),
    }
}
