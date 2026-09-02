use storyos_application::{
    ApplyAuthorEditCommand, ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError,
    ApplyAuthorEditReconfirmationReason, AuthorCommandAdmissionIds, AuthorEditError,
    AuthorEditSettlement, CommittedApplyAuthorEdit, EditorSessionId,
    ProjectCommandChallengeBinding, ReadApplyAuthorEditOutcome,
    RequiresReconfirmationApplyAuthorEdit,
};
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};
use uuid::Uuid;

use super::author_edit::{AuthorEditFault, parse_u64};
use super::*;

pub(super) struct OpenAdmission {
    pub command_id: String,
    pub author_command_admission_id: String,
    pub canonical_command_digest: String,
    pub method: String,
    pub route_template: String,
    pub command_schema: String,
    pub payload: String,
    pub payload_complete: bool,
    pub bindings_match: bool,
    pub unexpired: bool,
}

impl PostgresProjectReader {
    pub(super) async fn reconcile_open_apply_author_edit(
        &self,
        query: &ReadApplyAuthorEditOutcome,
        admission: OpenAdmission,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError> {
        if let Some(reason) = reconfirmation_reason(&admission) {
            return self
                .persist_requires_reconfirmation(query, &admission, reason)
                .await;
        }
        let Some(command) = reconstruct_command(query, &admission) else {
            return self
                .persist_requires_reconfirmation(
                    query,
                    &admission,
                    ApplyAuthorEditReconfirmationReason::DirectEditIntentUnrecoverable,
                )
                .await;
        };
        match self
            .complete_admitted_author_edit(&command, AuthorEditFault::None)
            .await
        {
            Ok(settlement) => Ok(committed_outcome(&command, settlement)),
            Err(AuthorEditError::AdmissionExpired) => {
                self.persist_requires_reconfirmation(
                    query,
                    &admission,
                    ApplyAuthorEditReconfirmationReason::AdmissionExpired,
                )
                .await
            }
            Err(AuthorEditError::StaleWriter | AuthorEditError::BindingConflict) => {
                if let Ok(reconfirmation) = self
                    .read_requires_reconfirmation(
                        query,
                        &admission.command_id,
                        &admission.author_command_admission_id,
                    )
                    .await
                {
                    return Ok(reconfirmation);
                }
                self.persist_requires_reconfirmation(
                    query,
                    &admission,
                    ApplyAuthorEditReconfirmationReason::BindingChanged,
                )
                .await
            }
            Err(error @ (AuthorEditError::InvalidChallenge | AuthorEditError::Unavailable(_))) => {
                Err(ApplyAuthorEditOutcomeReadError::unavailable(error))
            }
        }
    }

    pub(super) async fn read_requires_reconfirmation(
        &self,
        query: &ReadApplyAuthorEditOutcome,
        command_id: &str,
        author_command_admission_id: &str,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
        set_challenge_scope_on_client(&client, &query.project_scope)
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
        let row = client
            .query_opt(
                "SELECT reconfirmation.reconfirmation_reason,
                        reconfirmation.recovery_draft_ref::text
                   FROM storyos.author_command_admission_reconfirmations AS reconfirmation
                  WHERE reconfirmation.owner_user_id = $1::text::uuid
                    AND reconfirmation.project_id = $2::text::uuid
                    AND reconfirmation.author_command_admission_id = $3::text::uuid",
                &[
                    &query.project_scope.owner_user_id.as_ref(),
                    &query.project_scope.project_id.as_ref(),
                    &author_command_admission_id,
                ],
            )
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?
            .ok_or_else(outcome_unavailable)?;
        Ok(ApplyAuthorEditOutcome::RequiresReconfirmation(
            RequiresReconfirmationApplyAuthorEdit {
                command_id: command_id.to_owned(),
                author_command_admission_id: author_command_admission_id.to_owned(),
                reconfirmation_reason: parse_reason(row.get(0)).ok_or_else(outcome_unavailable)?,
                recovery_draft_ref: row.get(1),
            },
        ))
    }

    pub(super) async fn persist_requires_reconfirmation_in_tx(
        &self,
        client: &tokio_postgres::Client,
        query: &ReadApplyAuthorEditOutcome,
        admission: &OpenAdmission,
        reason: ApplyAuthorEditReconfirmationReason,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError> {
        let reason_text = reason_text(&reason);
        let inserted = client
            .execute(
                "INSERT INTO storyos.author_command_admission_reconfirmations
                   (owner_user_id, project_id, author_command_admission_id, command_id,
                    reconfirmation_reason)
                 SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, admission.command_id, $4
                   FROM storyos.command_idempotency AS idempotency
                   JOIN storyos.author_command_admissions AS admission
                     ON admission.owner_user_id = $1::text::uuid
                    AND admission.project_id = $2::text::uuid
                    AND admission.author_command_admission_id = $3::text::uuid
                  WHERE idempotency.owner_user_id = $1::text::uuid
                    AND idempotency.project_id = $2::text::uuid
                    AND idempotency.command_kind = 'applyAuthorEdit'
                    AND idempotency.idempotency_key = $5::text::uuid
                    AND idempotency.outcome_kind = 'in_progress'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
                       WHERE settlement.owner_user_id = $1::text::uuid
                         AND settlement.project_id = $2::text::uuid
                         AND settlement.author_command_admission_id = $3::text::uuid
                    )
                 ON CONFLICT (owner_user_id, project_id, author_command_admission_id)
                 DO NOTHING",
                &[
                    &query.project_scope.owner_user_id.as_ref(),
                    &query.project_scope.project_id.as_ref(),
                    &admission.author_command_admission_id,
                    &reason_text,
                    &query.idempotency_key,
                ],
            )
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
        if inserted == 1 {
            let updated = client
                .execute(
                    "UPDATE storyos.command_idempotency
                        SET outcome_kind = 'settled',
                            result_reference = 'requires_reconfirmation'
                      WHERE owner_user_id = $1::text::uuid
                        AND project_id = $2::text::uuid
                        AND command_kind = 'applyAuthorEdit'
                        AND idempotency_key = $3::text::uuid
                        AND outcome_kind = 'in_progress'",
                    &[
                        &query.project_scope.owner_user_id.as_ref(),
                        &query.project_scope.project_id.as_ref(),
                        &query.idempotency_key,
                    ],
                )
                .await
                .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
            if updated != 1 {
                return Err(outcome_unavailable());
            }
            return Ok(ApplyAuthorEditOutcome::RequiresReconfirmation(
                RequiresReconfirmationApplyAuthorEdit {
                    command_id: admission.command_id.clone(),
                    author_command_admission_id: admission.author_command_admission_id.clone(),
                    reconfirmation_reason: reason,
                    recovery_draft_ref: None,
                },
            ));
        }
        let row = client
            .query_opt(
                "SELECT reconfirmation.reconfirmation_reason,
                        reconfirmation.recovery_draft_ref::text
                   FROM storyos.author_command_admission_reconfirmations AS reconfirmation
                  WHERE reconfirmation.owner_user_id = $1::text::uuid
                    AND reconfirmation.project_id = $2::text::uuid
                    AND reconfirmation.author_command_admission_id = $3::text::uuid",
                &[
                    &query.project_scope.owner_user_id.as_ref(),
                    &query.project_scope.project_id.as_ref(),
                    &admission.author_command_admission_id,
                ],
            )
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?
            .ok_or_else(outcome_unavailable)?;
        Ok(ApplyAuthorEditOutcome::RequiresReconfirmation(
            RequiresReconfirmationApplyAuthorEdit {
                command_id: admission.command_id.clone(),
                author_command_admission_id: admission.author_command_admission_id.clone(),
                reconfirmation_reason: parse_reason(row.get(0)).ok_or_else(outcome_unavailable)?,
                recovery_draft_ref: row.get(1),
            },
        ))
    }

    async fn persist_requires_reconfirmation(
        &self,
        query: &ReadApplyAuthorEditOutcome,
        admission: &OpenAdmission,
        reason: ApplyAuthorEditReconfirmationReason,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
        client
            .batch_execute("BEGIN")
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
        set_challenge_scope_on_client(&client, &query.project_scope)
            .await
            .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
        match self
            .persist_requires_reconfirmation_in_tx(&client, query, admission, reason)
            .await
        {
            Ok(outcome) => {
                client
                    .batch_execute("COMMIT")
                    .await
                    .map_err(ApplyAuthorEditOutcomeReadError::unavailable)?;
                Ok(outcome)
            }
            Err(error) => {
                client.batch_execute("ROLLBACK").await.ok();
                Err(error)
            }
        }
    }
}

pub(super) fn reconfirmation_reason(
    admission: &OpenAdmission,
) -> Option<ApplyAuthorEditReconfirmationReason> {
    if !admission.unexpired {
        return Some(ApplyAuthorEditReconfirmationReason::AdmissionExpired);
    }
    if !admission.payload_complete {
        return Some(ApplyAuthorEditReconfirmationReason::DirectEditIntentUnrecoverable);
    }
    if !admission.bindings_match {
        return Some(ApplyAuthorEditReconfirmationReason::BindingChanged);
    }
    None
}

fn reconstruct_command(
    query: &ReadApplyAuthorEditOutcome,
    admission: &OpenAdmission,
) -> Option<ApplyAuthorEditCommand> {
    let payload: serde_json::Value = serde_json::from_str(&admission.payload).ok()?;
    let units = payload.get("author_edit_units").and_then(parse_units)?;
    if units.is_empty() {
        return None;
    }
    Some(ApplyAuthorEditCommand {
        project_scope: query.project_scope.clone(),
        client_binding: query.client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: query.project_scope.clone(),
            client_session_binding_digest: query.client_binding.binding_ref.clone(),
            client_session_generation: query.client_binding.session_generation,
            client_contract_revision: query.client_binding.client_contract_revision.clone(),
            security_policy_revision: query.client_binding.security_policy_revision.clone(),
            limit_profile_revision: query.limit_profile_revision.clone(),
            challenge_rate_policy_revision:
                storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
            method: admission.method.clone(),
            route_template: admission.route_template.clone(),
            command_schema: admission.command_schema.clone(),
            command_kind: "applyAuthorEdit".to_owned(),
            canonical_command_digest: admission.canonical_command_digest.clone(),
            idempotency_key: query.idempotency_key.clone(),
        },
        nonce_digest: query.nonce_digest.clone(),
        canonical_command_bytes: admission.payload.as_bytes().to_vec(),
        correlation_id: required_string(&payload, "correlation_id")?,
        ids: AuthorCommandAdmissionIds {
            command_id: admission.command_id.clone(),
            author_command_admission_id: admission.author_command_admission_id.clone(),
            receipt_id: Uuid::now_v7().to_string(),
        },
        editor_session_id: EditorSessionId::new(required_string(&payload, "editor_session_id")?),
        writer_generation: parse_u64(required_string(&payload, "writer_generation")?).ok()?,
        chapter_id: required_string(&payload, "chapter_id")?,
        expected_authoritative_revision_id: required_string(
            &payload,
            "expected_authoritative_revision_id",
        )?,
        expected_proposal_head_revision_ids: string_array(
            payload.get("expected_proposal_head_revision_ids")?,
        )?,
        target_refs: string_array(payload.get("target_refs")?)?,
        observed_ownership_partition: required_string(&payload, "observed_ownership_partition")?,
        editor_contract_revision: required_string(&payload, "editor_contract_revision")?,
        undo_group_id: required_string(&payload, "undo_group_id")?,
        completed_intent_record_id: required_string(&payload, "completed_intent_record_id")?,
        local_intent_sequence: parse_u64(required_string(&payload, "local_intent_sequence")?)
            .ok()?,
        author_edit_units: units,
    })
}

fn committed_outcome(
    command: &ApplyAuthorEditCommand,
    settlement: AuthorEditSettlement,
) -> ApplyAuthorEditOutcome {
    ApplyAuthorEditOutcome::Committed(Box::new(CommittedApplyAuthorEdit {
        correlation_id: command.correlation_id.clone(),
        canonical_command_digest: command.challenge_binding.canonical_command_digest.clone(),
        idempotency_key: command.challenge_binding.idempotency_key.clone(),
        expected_authoritative_revision_id: command.expected_authoritative_revision_id.clone(),
        settlement,
    }))
}

fn parse_units(value: &serde_json::Value) -> Option<Vec<AuthorEditUnit>> {
    value
        .as_array()?
        .iter()
        .map(|unit| {
            let snapshot = unit.get("selection_snapshot")?;
            Some(AuthorEditUnit {
                normalized_primitives: unit
                    .get("normalized_primitives")?
                    .as_array()?
                    .iter()
                    .map(parse_primitive)
                    .collect::<Option<Vec<_>>>()?,
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: required_string(snapshot, "coordinate_profile")?,
                    from: snapshot.get("from")?.as_u64()? as u32,
                    to: snapshot.get("to")?.as_u64()? as u32,
                },
            })
        })
        .collect()
}

fn parse_primitive(value: &serde_json::Value) -> Option<AuthorEditPrimitive> {
    match value.get("kind")?.as_str()? {
        "replace_selection" => Some(AuthorEditPrimitive::ReplaceSelection {
            from: value.get("from")?.as_u64()? as u32,
            to: value.get("to")?.as_u64()? as u32,
            text: required_string(value, "text")?,
        }),
        "replace_block_selection" => Some(AuthorEditPrimitive::ReplaceBlockSelection {
            manuscript_block_id: required_string(value, "manuscript_block_id")?,
            from: value.get("from")?.as_u64()? as u32,
            to: value.get("to")?.as_u64()? as u32,
            text: required_string(value, "text")?,
        }),
        "split_block" => Some(AuthorEditPrimitive::SplitBlock {
            manuscript_block_id: required_string(value, "manuscript_block_id")?,
            offset: value.get("offset")?.as_u64()? as u32,
            new_manuscript_block_id: required_string(value, "new_manuscript_block_id")?,
        }),
        "join_blocks" => Some(AuthorEditPrimitive::JoinBlocks {
            left_manuscript_block_id: required_string(value, "left_manuscript_block_id")?,
            right_manuscript_block_id: required_string(value, "right_manuscript_block_id")?,
        }),
        "move_block" => Some(AuthorEditPrimitive::MoveBlock {
            manuscript_block_id: required_string(value, "manuscript_block_id")?,
            to_index: value.get("to_index")?.as_u64()? as u32,
        }),
        "retype_block" => Some(AuthorEditPrimitive::RetypeBlock {
            manuscript_block_id: required_string(value, "manuscript_block_id")?,
            block_kind: match value.get("block_kind")?.as_str()? {
                "paragraph" => storyos_core::ManuscriptBlockKind::Paragraph,
                "heading" => storyos_core::ManuscriptBlockKind::Heading,
                _ => return None,
            },
        }),
        _ => None,
    }
}

fn required_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(ToOwned::to_owned)
}

fn string_array(value: &serde_json::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(ToOwned::to_owned))
        .collect()
}

fn reason_text(reason: &ApplyAuthorEditReconfirmationReason) -> &'static str {
    match reason {
        ApplyAuthorEditReconfirmationReason::AdmissionExpired => "admission_expired",
        ApplyAuthorEditReconfirmationReason::BindingChanged => "binding_changed",
        ApplyAuthorEditReconfirmationReason::DirectEditIntentUnrecoverable => {
            "direct_edit_intent_unrecoverable"
        }
    }
}

fn parse_reason(value: String) -> Option<ApplyAuthorEditReconfirmationReason> {
    match value.as_str() {
        "admission_expired" => Some(ApplyAuthorEditReconfirmationReason::AdmissionExpired),
        "binding_changed" => Some(ApplyAuthorEditReconfirmationReason::BindingChanged),
        "direct_edit_intent_unrecoverable" => {
            Some(ApplyAuthorEditReconfirmationReason::DirectEditIntentUnrecoverable)
        }
        _ => None,
    }
}

fn outcome_unavailable() -> ApplyAuthorEditOutcomeReadError {
    ApplyAuthorEditOutcomeReadError::unavailable(std::io::Error::other(
        "Apply Author Edit outcome relation is unavailable",
    ))
}
