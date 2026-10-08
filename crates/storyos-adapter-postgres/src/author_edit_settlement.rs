use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditError, AuthorEditSettlement, AuthorEditSettlementEffect,
    AuthoritativeAppliedIds,
};
use storyos_core::{
    AuthorEditApplied, AuthorEditConflict, AuthorEditNoEffect, AuthorEditRefusal,
    AuthorEditRefused, ManuscriptBlock, ReasonCode, TransitionOutcome,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::author_edit::{ClassifiedAuthorEdit, author_edit_database_error, parse_u64, sha256_hex};

pub(super) async fn persist_author_edit_settlement(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    current_revision_id: &str,
    classified: ClassifiedAuthorEdit,
    source_disposition: Option<storyos_contracts::SourceDraftDisposition>,
) -> Result<AuthorEditSettlement, AuthorEditError> {
    let prepared = match classified.result {
        TransitionOutcome::Applied(AuthorEditApplied::AuthoritativeApplied { body }) => {
            let persist_body = classified
                .successor_blocks
                .as_ref()
                .map(|blocks| crate::manuscript_block::persist_canonical_bytes(blocks))
                .unwrap_or(body);
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
                .map_err(author_edit_database_error)?;
            let author_action_sequence = parse_u64(counter_row.get(0))?;
            let authoritative_commit_sequence = parse_u64(counter_row.get(1))?;
            let project_activity_position = parse_u64(counter_row.get(2))?;
            let ids = AuthoritativeAppliedIds {
                revision_id: Uuid::now_v7().to_string(),
                payload_id: Uuid::now_v7().to_string(),
                authoritative_commit_id: Uuid::now_v7().to_string(),
                project_activity_event_id: Uuid::now_v7().to_string(),
            };
            persist_authority_change(
                client,
                command,
                current_revision_id,
                &ids,
                &persist_body,
                classified.successor_blocks.as_deref(),
                authoritative_commit_sequence,
            )
            .await?;
            let blocks = crate::manuscript_block::load_revision_blocks(
                client,
                command.project_scope.owner_user_id.as_ref(),
                command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &ids.revision_id,
                &persist_body,
            )
            .await
            .map_err(author_edit_database_error)?;
            if blocks.is_empty() {
                return Err(AuthorEditError::Unavailable(Box::new(
                    std::io::Error::other("successor revision members were not copied"),
                )));
            }
            if let Some(context) = classified.proposal_context.as_ref()
                && context.kind == "inline_edit"
            {
                super::author_edit_proposal::append_proposal_revision(
                    client,
                    &command.project_scope,
                    context,
                    &ids.revision_id,
                    &context.candidate_text,
                )
                .await?;
            }
            PreparedSettlement::AuthoritativeApplied {
                ids,
                body: crate::manuscript_block::display_body_from_stored(&persist_body, &blocks),
                blocks,
                author_action_sequence,
                project_activity_position,
            }
        }
        TransitionOutcome::Applied(AuthorEditApplied::ProposalRevised { candidate_text }) => {
            let Some(context) = classified.proposal_context.as_ref() else {
                return Err(AuthorEditError::BindingConflict);
            };
            let proposal_revision_id = super::author_edit_proposal::append_proposal_revision(
                client,
                &command.project_scope,
                context,
                current_revision_id,
                &candidate_text,
            )
            .await?;
            let counter_row = client
                .query_one(
                    "UPDATE storyos.scope_counters
                        SET author_action_sequence = author_action_sequence + 1
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  RETURNING author_action_sequence::text",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                    ],
                )
                .await
                .map_err(author_edit_database_error)?;
            PreparedSettlement::ProposalRevised {
                proposal_revision_id,
                author_action_sequence: parse_u64(counter_row.get(0))?,
            }
        }
        TransitionOutcome::Refused(AuthorEditRefused::RefusedToDraft) => {
            PreparedSettlement::RefusedToDraft {
                identity: crate::refused_edit_draft::persist(client, command).await?,
            }
        }
        TransitionOutcome::NoEffect(reason) => PreparedSettlement::NoEffect { reason },
        TransitionOutcome::Conflicted(reason) => PreparedSettlement::Conflicted {
            reason,
            current_authoritative_revision_id: current_revision_id.to_owned(),
        },
        TransitionOutcome::Refused(AuthorEditRefused::Refused(reason)) => {
            PreparedSettlement::Refused { reason }
        }
    };

    let (
        result_kind,
        result_payload,
        resulting_head,
        revision_ids,
        proposal_revision_ids,
        commit_ids,
    ) = match &prepared {
        PreparedSettlement::AuthoritativeApplied { ids, .. } => (
            "authoritative_applied",
            serde_json::json!({}),
            ids.revision_id.as_str(),
            vec![ids.revision_id.clone()],
            Vec::new(),
            vec![ids.authoritative_commit_id.clone()],
        ),
        PreparedSettlement::ProposalRevised {
            proposal_revision_id,
            ..
        } => (
            "proposal_revised",
            serde_json::json!({}),
            current_revision_id,
            Vec::new(),
            vec![proposal_revision_id.clone()],
            Vec::new(),
        ),
        PreparedSettlement::RefusedToDraft { identity } => (
            "refused_to_draft",
            serde_json::json!({"draft_id": identity.draft_id, "draft_revision_id": identity.draft_revision_id, "creation_event_id": identity.creation_event_id}),
            current_revision_id,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        PreparedSettlement::NoEffect { reason } => (
            "no_effect",
            serde_json::json!({"reason": reason.code()}),
            current_revision_id,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        PreparedSettlement::Conflicted {
            reason,
            current_authoritative_revision_id,
        } => (
            "conflicted",
            serde_json::json!({
                "reason": reason.code(),
                "current_authoritative_revision_id": current_authoritative_revision_id,
            }),
            current_revision_id,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        PreparedSettlement::Refused { reason } => (
            "refused",
            serde_json::json!({"reason": reason.code()}),
            current_revision_id,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    };
    let source_draft_disposition = super::draft_retry::supersede_source(
        client,
        command,
        source_disposition,
        result_kind,
        match &prepared {
            PreparedSettlement::AuthoritativeApplied {
                author_action_sequence,
                ..
            }
            | PreparedSettlement::ProposalRevised {
                author_action_sequence,
                ..
            } => Some(*author_action_sequence),
            _ => None,
        },
    )
    .await?;
    let replacement_provenance = if result_kind == "refused_to_draft" {
        super::draft_retry::replacement_provenance(
            source_draft_disposition.as_ref(),
            command.retry_source.as_ref(),
        )
    } else {
        None
    };
    let source_json = source_draft_disposition
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    let result_payload = result_payload.to_string();
    let mut draft_refs = match &prepared {
        PreparedSettlement::RefusedToDraft { identity } => vec![identity.draft_id.clone()],
        _ => Vec::new(),
    };
    let mut lifecycle_refs = match &prepared {
        PreparedSettlement::RefusedToDraft { identity } => vec![identity.creation_event_id.clone()],
        _ => Vec::new(),
    };
    if let Some(storyos_contracts::SourceDraftDisposition::ClosedSuperseded {
        source_draft_id,
        closure_event_ref,
        ..
    }) = &source_draft_disposition
    {
        draft_refs.push(source_draft_id.clone());
        lifecycle_refs.push(closure_event_ref.clone());
    }
    let receipt_row = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids,
                draft_artifact_refs, artifact_lifecycle_event_refs, condition_refs,
                result_kind, result_payload, source_draft_disposition, created_at)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'applyAuthorEdit', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$9::text::uuid],
                     ARRAY[$10::text::uuid],
                     $11::text[]::uuid[], $12::text[]::uuid[], $13::text[]::uuid[],
                     $16::text[], $17::text[], ARRAY[]::text[],
                     $14, $15::text::jsonb, $18::text::jsonb,
                     CASE WHEN $18::text IS NULL THEN clock_timestamp() ELSE transaction_timestamp() END)
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
                &current_revision_id,
                &resulting_head,
                &revision_ids,
                &proposal_revision_ids,
                &commit_ids,
                &result_kind,
                &result_payload.to_string(),
                &draft_refs,
                &lifecycle_refs,
                &source_json,
            ],
        )
        .await
        .map_err(persist_conflict_or_unavailable)?;
    let receipt_created_at = receipt_row.get::<_, String>(0);

    let effect = match prepared {
        PreparedSettlement::AuthoritativeApplied {
            ids,
            body,
            blocks,
            author_action_sequence,
            project_activity_position,
        } => {
            client
                .execute(
                    "INSERT INTO storyos.author_action_entries
                   (owner_user_id, project_id, author_action_sequence, disposition,
                    authoritative_commit_id, receipt_id, receipt_result_kind)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                         $4::text::uuid, $5::text::uuid, 'authoritative_applied')",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &author_action_sequence.to_string(),
                        &ids.authoritative_commit_id,
                        &command.ids.receipt_id,
                    ],
                )
                .await
                .map_err(author_edit_database_error)?;
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
                        &ids.project_activity_event_id,
                        &command.ids.receipt_id,
                        &ids.authoritative_commit_id,
                        &ids.revision_id,
                        &author_action_sequence.to_string(),
                    ],
                )
                .await
                .map_err(author_edit_database_error)?;
            let base_snapshot_id = Uuid::now_v7().to_string();
            let base_snapshot_updates = client
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
                             writer.current_editor_session_id, writer.writer_generation) =
                            (snapshot.owner_user_id, snapshot.project_id,
                             snapshot.editor_session_id, $8::text::numeric)",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &command.editor_session_id.as_ref(),
                        &base_snapshot_id,
                        &ids.revision_id,
                        &project_activity_position.to_string(),
                        &current_revision_id,
                        &command.writer_generation.to_string(),
                    ],
                )
                .await
                .map_err(author_edit_database_error)?;
            if base_snapshot_updates != 1 {
                return Err(AuthorEditError::BindingConflict);
            }
            crate::snapshot::persist_canonical_snapshot(
                client,
                &command.project_scope,
                &base_snapshot_id,
                project_activity_position,
            )
            .await
            .map_err(author_edit_database_error)?;
            AuthorEditSettlementEffect::AuthoritativeApplied {
                ids,
                body,
                blocks,
                author_action_sequence,
                project_activity_position,
            }
        }
        PreparedSettlement::ProposalRevised {
            proposal_revision_id,
            author_action_sequence,
        } => {
            client
                .execute(
                    "INSERT INTO storyos.author_action_entries
                   (owner_user_id, project_id, author_action_sequence, disposition,
                    receipt_id, receipt_result_kind)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                         $4::text::uuid, 'proposal_revised')",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &author_action_sequence.to_string(),
                        &command.ids.receipt_id,
                    ],
                )
                .await
                .map_err(author_edit_database_error)?;
            AuthorEditSettlementEffect::ProposalRevised {
                proposal_revision_id,
                author_action_sequence,
            }
        }
        PreparedSettlement::RefusedToDraft { identity } => {
            AuthorEditSettlementEffect::RefusedToDraft { identity }
        }
        PreparedSettlement::NoEffect { reason } => AuthorEditSettlementEffect::NoEffect { reason },
        PreparedSettlement::Conflicted {
            reason,
            current_authoritative_revision_id,
        } => AuthorEditSettlementEffect::Conflicted {
            reason,
            current_authoritative_revision_id,
        },
        PreparedSettlement::Refused { reason } => AuthorEditSettlementEffect::Refused { reason },
    };
    let settlement_inserts = client
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
        .map_err(persist_conflict_or_unavailable)?;
    if settlement_inserts != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
    let idempotency_updates = client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'applyAuthorEdit' AND idempotency_key = $4::text::uuid
                AND outcome_kind = 'in_progress'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if idempotency_updates != 1 {
        return Err(AuthorEditError::BindingConflict);
    }

    Ok(AuthorEditSettlement {
        ids: command.ids.clone(),
        effect,
        source_draft_disposition,
        replacement_provenance,
        receipt_created_at,
        completed_intent_record_id: command.completed_intent_record_id.clone(),
        local_intent_sequence: command.local_intent_sequence,
    })
}

async fn persist_authority_change(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    current_revision_id: &str,
    ids: &AuthoritativeAppliedIds,
    body: &str,
    successor_blocks: Option<&[ManuscriptBlock]>,
    authoritative_commit_sequence: u64,
) -> Result<(), AuthorEditError> {
    client
        .execute(
            "INSERT INTO storyos.authoritative_payloads
           (owner_user_id, project_id, payload_id, canonical_bytes)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8'))",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &ids.payload_id,
                &body,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revisions
           (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &ids.revision_id,
                &ids.payload_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let copied = if let Some(blocks) = successor_blocks {
        crate::manuscript_block::persist_revision_members_from_blocks(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
            &command.chapter_id,
            &ids.revision_id,
            blocks,
        )
        .await
        .map_err(author_edit_database_error)?
    } else {
        crate::manuscript_block::copy_or_upgrade_revision_members(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
            &command.chapter_id,
            current_revision_id,
            &ids.revision_id,
        )
        .await
        .map_err(author_edit_database_error)?
    };
    if copied == 0 {
        return Err(AuthorEditError::Unavailable(Box::new(
            std::io::Error::other("successor revision members were not copied"),
        )));
    }
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
                &command.chapter_id,
                &ids.revision_id,
                &current_revision_id,
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
                &sha256_hex(body.as_bytes()),
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.authoritative_heads SET current_revision_id = $4::text::uuid
          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
            AND manuscript_object_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &ids.revision_id,
                &current_revision_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if head_updates != 1 {
        return Err(AuthorEditError::BindingConflict);
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
                &ids.authoritative_commit_id,
                &authoritative_commit_sequence.to_string(),
                &command.chapter_id,
                &current_revision_id,
                &ids.revision_id,
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    Ok(())
}

enum PreparedSettlement {
    RefusedToDraft {
        identity: storyos_application::RefusedEditDraftIdentity,
    },
    AuthoritativeApplied {
        ids: AuthoritativeAppliedIds,
        body: String,
        blocks: Vec<storyos_core::ManuscriptBlock>,
        author_action_sequence: u64,
        project_activity_position: u64,
    },
    ProposalRevised {
        proposal_revision_id: String,
        author_action_sequence: u64,
    },
    NoEffect {
        reason: AuthorEditNoEffect,
    },
    Conflicted {
        reason: AuthorEditConflict,
        current_authoritative_revision_id: String,
    },
    Refused {
        reason: AuthorEditRefusal,
    },
}

fn persist_conflict_or_unavailable(error: tokio_postgres::Error) -> AuthorEditError {
    if error.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION)
        || error.code() == Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    {
        AuthorEditError::BindingConflict
    } else {
        author_edit_database_error(error)
    }
}
