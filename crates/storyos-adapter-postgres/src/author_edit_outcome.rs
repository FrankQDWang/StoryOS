use storyos_application::{
    ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError, ApplyAuthorEditOutcomeReader,
    ApplyAuthorEditRejectionReason, ApplyAuthorEditUnknownObservation, AuthorEditError,
    CommittedApplyAuthorEdit, ReadApplyAuthorEditOutcome,
};

use super::author_edit::sha256_hex;
use super::author_edit_admission_recovery::OpenAdmission;
use super::author_edit_replay::AuthorEditReplayIdentity;
use super::*;

impl ApplyAuthorEditOutcomeReader for PostgresProjectReader {
    async fn read_apply_author_edit_outcome(
        &self,
        query: &ReadApplyAuthorEditOutcome,
    ) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeReadError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(outcome_store_error)?;
        client
            .batch_execute("BEGIN")
            .await
            .map_err(outcome_database_error)?;
        set_challenge_scope_on_client(&client, &query.project_scope)
            .await
            .map_err(outcome_store_error)?;
        let arbiter = client
            .query_opt(
                "SELECT challenge.client_session_binding_digest,
                        challenge.client_session_generation::text,
                        challenge.client_contract_revision,
                        challenge.security_policy_revision,
                        challenge.limit_profile_revision,
                        challenge.challenge_rate_policy_revision,
                        challenge.method,
                        challenge.route_template,
                        challenge.command_schema,
                        challenge.canonical_command_digest,
                        challenge.nonce_digest,
                        to_char(challenge.expires_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        challenge.expires_at > clock_timestamp(),
                        challenge.consumed_at IS NOT NULL,
                        idempotency.canonical_command_digest,
                        idempotency.outcome_kind,
                        idempotency.result_reference
                   FROM storyos.project_command_challenges AS challenge
                   JOIN storyos.command_idempotency AS idempotency USING
                     (owner_user_id, project_id, command_kind, idempotency_key)
                  WHERE challenge.owner_user_id = $1::text::uuid
                    AND challenge.project_id = $2::text::uuid
                    AND challenge.command_kind = 'applyAuthorEdit'
                    AND challenge.idempotency_key = $3::text::uuid
                  FOR UPDATE OF challenge, idempotency",
                &[
                    &query.project_scope.owner_user_id.as_ref(),
                    &query.project_scope.project_id.as_ref(),
                    &query.idempotency_key,
                ],
            )
            .await
            .map_err(outcome_database_error)?
            .ok_or_else(outcome_unavailable)?;

        let session_generation = query.client_binding.session_generation.to_string();
        let exact_binding = arbiter.get::<_, String>(0) == query.client_binding.binding_ref
            && arbiter.get::<_, String>(1) == session_generation
            && arbiter.get::<_, String>(2) == query.client_binding.client_contract_revision
            && arbiter.get::<_, String>(3) == query.client_binding.security_policy_revision
            && arbiter.get::<_, String>(4) == query.limit_profile_revision
            && arbiter.get::<_, String>(5)
                == storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION
            && arbiter.get::<_, String>(6) == "POST"
            && arbiter.get::<_, String>(7)
                == "/api/v1/projects/{project_id}/manuscript/author-edits"
            && arbiter.get::<_, String>(8) == "storyos.command.apply-author-edit.request.v1"
            && arbiter.get::<_, String>(9) == arbiter.get::<_, String>(14)
            && arbiter.get::<_, String>(10) == query.nonce_digest;
        if !exact_binding {
            return Err(outcome_unavailable());
        }

        let relation = read_outcome_relation(&client, query).await?;
        let unexpired = arbiter.get::<_, bool>(12);
        let consumed = arbiter.get::<_, bool>(13);
        let idempotency_outcome = arbiter.get::<_, String>(15);
        let result_reference = arbiter.get::<_, Option<String>>(16);
        let admission_count = relation.get::<_, i64>(0);
        let receipt_count = relation.get::<_, i64>(1);
        let payload_digest_exact = relation
            .get::<_, Option<String>>(12)
            .and_then(|payload| canonical_command_digest(&payload))
            == Some(arbiter.get::<_, String>(9));
        let clean_pre_admission = !consumed
            && idempotency_outcome == "pending"
            && result_reference.is_none()
            && admission_count == 0
            && receipt_count == 0;
        let open_admission = consumed
            && idempotency_outcome == "in_progress"
            && result_reference.is_none()
            && admission_count == 1
            && receipt_count == 0;
        let reconfirmed = consumed
            && idempotency_outcome == "settled"
            && result_reference.as_deref() == Some("requires_reconfirmation")
            && admission_count == 1
            && receipt_count == 0;
        let clean_settlement = consumed
            && idempotency_outcome == "settled"
            && result_reference.is_some()
            && result_reference.as_deref() != Some("requires_reconfirmation")
            && admission_count == 1
            && receipt_count == 1
            && relation.get::<_, Option<bool>>(10) == Some(true)
            && relation.get::<_, Option<bool>>(11) == Some(true)
            && payload_digest_exact;
        if clean_settlement {
            let canonical_command_digest = relation
                .get::<_, Option<String>>(5)
                .ok_or_else(outcome_unavailable)?;
            let idempotency_key = relation
                .get::<_, Option<String>>(6)
                .ok_or_else(outcome_unavailable)?;
            let expected_authoritative_revision_id = relation
                .get::<_, Option<String>>(8)
                .ok_or_else(outcome_unavailable)?;
            let identity = AuthorEditReplayIdentity {
                project_scope: query.project_scope.clone(),
                canonical_command_digest: canonical_command_digest.clone(),
                idempotency_key: idempotency_key.clone(),
                chapter_id: relation
                    .get::<_, Option<String>>(7)
                    .ok_or_else(outcome_unavailable)?,
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
                target_refs: relation
                    .get::<_, Option<Vec<String>>>(9)
                    .ok_or_else(outcome_unavailable)?,
            };
            let correlation_id = relation
                .get::<_, Option<String>>(4)
                .ok_or_else(outcome_unavailable)?;
            let receipt_id = result_reference.ok_or_else(outcome_unavailable)?;
            client
                .batch_execute("COMMIT")
                .await
                .map_err(outcome_database_error)?;
            let settlement = self
                .read_author_edit_settlement_by_identity(&identity, &receipt_id)
                .await
                .map_err(outcome_settlement_error)?;
            return Ok(ApplyAuthorEditOutcome::Committed(Box::new(
                CommittedApplyAuthorEdit {
                    correlation_id,
                    canonical_command_digest,
                    idempotency_key,
                    expected_authoritative_revision_id,
                    settlement,
                },
            )));
        }
        let outcome = if clean_pre_admission && unexpired {
            ApplyAuthorEditOutcome::StillUnknown {
                observation: ApplyAuthorEditUnknownObservation::ChallengeIssued {
                    expires_at: arbiter.get(11),
                },
            }
        } else if clean_pre_admission {
            ApplyAuthorEditOutcome::Rejected {
                reason: ApplyAuthorEditRejectionReason::ChallengeExpiredUnconsumed,
            }
        } else if reconfirmed {
            let command_id = relation
                .get::<_, Option<String>>(2)
                .ok_or_else(outcome_unavailable)?;
            let author_command_admission_id = relation
                .get::<_, Option<String>>(3)
                .ok_or_else(outcome_unavailable)?;
            let outcome = super::author_edit_admission_recovery::load_requires_reconfirmation(
                &client,
                query,
                &command_id,
                &author_command_admission_id,
            )
            .await?;
            client
                .batch_execute("COMMIT")
                .await
                .map_err(outcome_database_error)?;
            return Ok(outcome);
        } else if open_admission {
            let command_id = relation
                .get::<_, Option<String>>(2)
                .ok_or_else(outcome_unavailable)?;
            let author_command_admission_id = relation
                .get::<_, Option<String>>(3)
                .ok_or_else(outcome_unavailable)?;
            let admission = OpenAdmission {
                command_id,
                author_command_admission_id,
                canonical_command_digest: arbiter.get(9),
                method: arbiter.get(6),
                route_template: arbiter.get(7),
                command_schema: arbiter.get(8),
                payload: relation.get::<_, Option<String>>(12).unwrap_or_default(),
                payload_complete: relation.get::<_, Option<bool>>(11) == Some(true)
                    && payload_digest_exact,
                bindings_match: relation.get::<_, Option<bool>>(10) == Some(true),
                unexpired,
            };
            if let Some(reason) =
                super::author_edit_admission_recovery::reconfirmation_reason(&admission)
            {
                match self
                    .persist_requires_reconfirmation_in_tx(&client, query, &admission, reason)
                    .await
                {
                    Ok(outcome) => {
                        client
                            .batch_execute("COMMIT")
                            .await
                            .map_err(outcome_database_error)?;
                        return Ok(outcome);
                    }
                    Err(error) => {
                        client.batch_execute("ROLLBACK").await.ok();
                        return Err(error);
                    }
                }
            }
            client
                .batch_execute("COMMIT")
                .await
                .map_err(outcome_database_error)?;
            return self
                .reconcile_open_apply_author_edit(query, admission)
                .await;
        } else if consumed
            && admission_count == 1
            && (idempotency_outcome == "in_progress" || receipt_count != 1)
        {
            ApplyAuthorEditOutcome::StillUnknown {
                observation: ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                    command_id: relation
                        .get::<_, Option<String>>(2)
                        .ok_or_else(outcome_unavailable)?,
                    author_command_admission_id: relation
                        .get::<_, Option<String>>(3)
                        .ok_or_else(outcome_unavailable)?,
                },
            }
        } else {
            return Err(outcome_unavailable());
        };
        client
            .batch_execute("COMMIT")
            .await
            .map_err(outcome_database_error)?;
        Ok(outcome)
    }
}

async fn read_outcome_relation(
    client: &tokio_postgres::Client,
    query: &ReadApplyAuthorEditOutcome,
) -> Result<tokio_postgres::Row, ApplyAuthorEditOutcomeReadError> {
    client
        .query_one(
            "SELECT (SELECT count(*)
                       FROM storyos.author_command_admissions AS candidate
                      WHERE candidate.owner_user_id = challenge.owner_user_id
                        AND candidate.project_id = challenge.project_id
                        AND candidate.command_kind = challenge.command_kind
                        AND candidate.idempotency_key = challenge.idempotency_key),
                    (SELECT count(*)
                       FROM storyos.domain_receipts AS receipt
                      WHERE receipt.owner_user_id = challenge.owner_user_id
                        AND receipt.project_id = challenge.project_id
                        AND receipt.command_kind = challenge.command_kind
                        AND receipt.idempotency_key = challenge.idempotency_key),
                    admission.command_id::text,
                    admission.author_command_admission_id::text,
                    admission.correlation_id::text,
                    admission.canonical_command_digest,
                    admission.idempotency_key::text,
                    admission.chapter_object_id::text,
                    admission.expected_authoritative_revision_id::text,
                    admission.target_refs,
                    admission.client_session_binding_ref =
                      challenge.client_session_binding_digest
                      AND admission.client_session_generation =
                        challenge.client_session_generation
                      AND admission.client_contract_revision =
                        challenge.client_contract_revision
                      AND admission.security_policy_revision =
                        challenge.security_policy_revision
                      AND admission.method = challenge.method
                      AND admission.route_template = challenge.route_template
                      AND admission.command_schema = challenge.command_schema
                      AND admission.command_kind = challenge.command_kind
                      AND admission.canonical_command_digest =
                        challenge.canonical_command_digest
                      AND admission.idempotency_key = challenge.idempotency_key
                      AND admission.challenge_consumed_at = challenge.consumed_at
                      AND admission.challenge_expires_at = challenge.expires_at,
                    (SELECT count(*)
                       FROM jsonb_object_keys(admission.command_payload)) = 16
                      AND admission.command_payload->>'command_schema' =
                        admission.command_schema
                      AND admission.command_payload->>'client_contract_revision' =
                        admission.client_contract_revision
                      AND admission.command_payload->>'security_policy_revision' =
                        admission.security_policy_revision
                      AND admission.command_payload->>'correlation_id' =
                        admission.correlation_id::text
                      AND admission.command_payload->>'editor_session_id' =
                        admission.editor_session_id::text
                      AND admission.command_payload->>'writer_generation' =
                        admission.writer_generation::text
                      AND admission.command_payload->>'chapter_id' =
                        admission.chapter_object_id::text
                      AND admission.command_payload->>'expected_authoritative_revision_id' =
                        admission.expected_authoritative_revision_id::text
                      AND admission.command_payload->'expected_proposal_head_revision_ids' =
                        to_jsonb(admission.expected_proposal_head_revision_ids::text[])
                      AND admission.command_payload->'target_refs' =
                        to_jsonb(admission.target_refs)
                      AND admission.command_payload->>'observed_ownership_partition' =
                        admission.observed_ownership_partition
                      AND admission.command_payload->>'editor_contract_revision' =
                        admission.editor_contract_revision
                      AND admission.command_payload->>'undo_group_id' =
                        admission.undo_group_id::text
                      AND admission.command_payload->>'completed_intent_record_id' =
                        admission.completed_intent_record_id::text
                      AND admission.command_payload->>'local_intent_sequence' =
                        admission.local_intent_sequence::text
                      AND jsonb_typeof(admission.command_payload->'author_edit_units') = 'array'
                      AND jsonb_array_length(admission.command_payload->'author_edit_units') > 0,
                    admission.command_payload::text
               FROM storyos.project_command_challenges AS challenge
               LEFT JOIN storyos.author_command_admissions AS admission USING
                 (owner_user_id, project_id, command_kind, idempotency_key)
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'applyAuthorEdit'
                AND challenge.idempotency_key = $3::text::uuid",
            &[
                &query.project_scope.owner_user_id.as_ref(),
                &query.project_scope.project_id.as_ref(),
                &query.idempotency_key,
            ],
        )
        .await
        .map_err(outcome_database_error)
}

fn canonical_command_digest(payload: &str) -> Option<String> {
    fn canonical_json(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
            }
            serde_json::Value::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, canonical_json(value)))
                    .collect(),
            ),
            scalar => scalar,
        }
    }

    let canonical = canonical_json(serde_json::from_str(payload).ok()?);
    let bytes = serde_json::to_vec(&canonical).ok()?;
    Some(format!(
        "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
        sha256_hex(&bytes)
    ))
}

fn outcome_store_error(
    error: storyos_application::ProjectCommandChallengeError,
) -> ApplyAuthorEditOutcomeReadError {
    ApplyAuthorEditOutcomeReadError::unavailable(error)
}

fn outcome_database_error(error: tokio_postgres::Error) -> ApplyAuthorEditOutcomeReadError {
    ApplyAuthorEditOutcomeReadError::unavailable(error)
}

fn outcome_settlement_error(error: AuthorEditError) -> ApplyAuthorEditOutcomeReadError {
    ApplyAuthorEditOutcomeReadError::unavailable(error)
}

fn outcome_unavailable() -> ApplyAuthorEditOutcomeReadError {
    ApplyAuthorEditOutcomeReadError::unavailable(std::io::Error::other(
        "Apply Author Edit outcome relation is unavailable",
    ))
}
