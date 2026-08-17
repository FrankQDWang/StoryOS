use storyos_application::{
    AppendAuthorCommandOutcomeUnknown, AuthorCommandOutcomeUnknownBoundary,
    AuthorCommandOutcomeUnknownError, AuthorCommandOutcomeUnknownObservation,
    AuthorCommandOutcomeUnknownReason, AuthorCommandOutcomeUnknownStore, ReconciliationRequired,
};

use super::*;

impl AuthorCommandOutcomeUnknownStore for PostgresProjectReader {
    async fn append_author_command_outcome_unknown(
        &self,
        request: &AppendAuthorCommandOutcomeUnknown,
    ) -> Result<AuthorCommandOutcomeUnknownObservation, AuthorCommandOutcomeUnknownError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
        client
            .batch_execute("BEGIN")
            .await
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
        set_challenge_scope_on_client(&client, &request.project_scope)
            .await
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;

        if let Some(observation) = read_observation(&client, request).await? {
            client
                .batch_execute("COMMIT")
                .await
                .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
            return exact_observation(request, observation);
        }

        let arbiter = client
            .query_opt(
                "SELECT idempotency.outcome_kind,
                        EXISTS (
                          SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
                           WHERE (settlement.owner_user_id, settlement.project_id, settlement.author_command_admission_id) =
                                 (admission.owner_user_id, admission.project_id, admission.author_command_admission_id))
                   FROM storyos.author_command_admissions AS admission
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id, idempotency.command_kind, idempotency.idempotency_key) =
                        (admission.owner_user_id, admission.project_id, admission.command_kind, admission.idempotency_key)
                  WHERE admission.owner_user_id = $1::text::uuid AND admission.project_id = $2::text::uuid
                    AND admission.author_command_admission_id = $3::text::uuid AND admission.command_kind = 'applyAuthorEdit'
                  FOR UPDATE OF idempotency",
                &[
                    &request.project_scope.owner_user_id.as_ref(),
                    &request.project_scope.project_id.as_ref(),
                    &request.author_command_admission_id,
                ],
            )
            .await
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;

        if let Some(observation) = read_observation(&client, request).await? {
            client
                .batch_execute("COMMIT")
                .await
                .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
            return exact_observation(request, observation);
        }
        let Some(arbiter) = arbiter else {
            client
                .batch_execute("ROLLBACK")
                .await
                .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
            return Err(AuthorCommandOutcomeUnknownError::BindingConflict);
        };
        if arbiter.get::<_, String>(0) != "in_progress" || arbiter.get::<_, bool>(1) {
            client
                .batch_execute("ROLLBACK")
                .await
                .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
            return Err(AuthorCommandOutcomeUnknownError::BindingConflict);
        }

        let row = client
            .query_one(
                "INSERT INTO storyos.author_command_admission_outcome_unknown_observations
                   (owner_user_id, project_id, observation_id, author_command_admission_id, last_provable_boundary, reason)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5, $6)
                 RETURNING observation_id::text, observation_sequence::text, author_command_admission_id::text, command_id::text,
                           command_kind, canonical_command_digest, idempotency_key::text,
                           last_provable_boundary, reason, reconciliation_required,
                           to_char(observed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
                &[
                    &request.project_scope.owner_user_id.as_ref(),
                    &request.project_scope.project_id.as_ref(),
                    &request.observation_id,
                    &request.author_command_admission_id,
                    &match request.last_provable_boundary {
                        AuthorCommandOutcomeUnknownBoundary::AdmissionCommitted => {
                            "admission_committed"
                        }
                    },
                    &match request.reason {
                        AuthorCommandOutcomeUnknownReason::AcknowledgementMissing => {
                            "acknowledgement_missing"
                        }
                    },
                ],
            )
            .await
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
        let observation = observation_from_row(&request.project_scope, row)?;
        client
            .batch_execute("COMMIT")
            .await
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?;
        Ok(observation)
    }
}

async fn read_observation(
    client: &tokio_postgres::Client,
    request: &AppendAuthorCommandOutcomeUnknown,
) -> Result<Option<AuthorCommandOutcomeUnknownObservation>, AuthorCommandOutcomeUnknownError> {
    client
        .query_opt(
            "SELECT observation_id::text, observation_sequence::text,
                    author_command_admission_id::text, command_id::text, command_kind, canonical_command_digest, idempotency_key::text,
                    last_provable_boundary, reason, reconciliation_required,
                    to_char(observed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
               FROM storyos.author_command_admission_outcome_unknown_observations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND observation_id = $3::text::uuid",
            &[
                &request.project_scope.owner_user_id.as_ref(),
                &request.project_scope.project_id.as_ref(),
                &request.observation_id,
            ],
        )
        .await
        .map_err(AuthorCommandOutcomeUnknownError::unavailable)?
        .map(|row| observation_from_row(&request.project_scope, row))
        .transpose()
}

fn exact_observation(
    request: &AppendAuthorCommandOutcomeUnknown,
    observation: AuthorCommandOutcomeUnknownObservation,
) -> Result<AuthorCommandOutcomeUnknownObservation, AuthorCommandOutcomeUnknownError> {
    if observation.author_command_admission_id == request.author_command_admission_id
        && observation.last_provable_boundary == request.last_provable_boundary
        && observation.reason == request.reason
    {
        Ok(observation)
    } else {
        Err(AuthorCommandOutcomeUnknownError::BindingConflict)
    }
}

fn observation_from_row(
    project_scope: &storyos_application::ProjectScope,
    row: tokio_postgres::Row,
) -> Result<AuthorCommandOutcomeUnknownObservation, AuthorCommandOutcomeUnknownError> {
    let boundary = match row.get::<_, String>(7).as_str() {
        "admission_committed" => AuthorCommandOutcomeUnknownBoundary::AdmissionCommitted,
        _ => return Err(outcome_unknown_unavailable()),
    };
    let reason = match row.get::<_, String>(8).as_str() {
        "acknowledgement_missing" => AuthorCommandOutcomeUnknownReason::AcknowledgementMissing,
        _ => return Err(outcome_unknown_unavailable()),
    };
    if row.get::<_, String>(4) != "applyAuthorEdit" || !row.get::<_, bool>(9) {
        return Err(outcome_unknown_unavailable());
    }
    Ok(AuthorCommandOutcomeUnknownObservation {
        project_scope: project_scope.clone(),
        observation_id: row.get(0),
        observation_sequence: row
            .get::<_, String>(1)
            .parse()
            .map_err(AuthorCommandOutcomeUnknownError::unavailable)?,
        author_command_admission_id: row.get(2),
        command_id: row.get(3),
        command_kind: row.get(4),
        canonical_command_digest: row.get(5),
        idempotency_key: row.get(6),
        last_provable_boundary: boundary,
        reason,
        reconciliation_required: ReconciliationRequired,
        observed_at: row.get(10),
    })
}

fn outcome_unknown_unavailable() -> AuthorCommandOutcomeUnknownError {
    AuthorCommandOutcomeUnknownError::unavailable(std::io::Error::other(
        "the durable outcome_unknown observation relation is invalid",
    ))
}
