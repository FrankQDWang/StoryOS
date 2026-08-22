//! PostgreSQL implementation of StoryOS Application ports.

#[cfg(test)]
#[path = "project_command_challenge_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "author_edit_outcome_tests.rs"]
mod author_edit_outcome_tests;

#[cfg(test)]
#[path = "takeover_persistence_tests.rs"]
mod takeover_persistence_tests;

#[cfg(test)]
#[path = "takeover_admission_tests.rs"]
mod takeover_admission_tests;

mod author_command_outcome_unknown;
mod author_edit;
mod author_edit_outcome;
mod author_edit_replay;
mod author_edit_settlement;
mod editor_session;
mod snapshot;
mod takeover;

use storyos_application::{
    Chapter, ChapterId, IssueProjectCommandChallenge, PROJECT_COMMAND_CHALLENGE_RATE_CAPACITY,
    PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION, PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS,
    Project, ProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectCommandChallengeError,
    ProjectCommandChallengeStore, ProjectCommandChallengeTransaction, ProjectCommandChallengeUse,
    ProjectId, ProjectReadError, ProjectReader, ProjectScope, RevisionId,
};
use tokio_postgres::NoTls;

#[derive(Clone, Debug)]
pub struct PostgresProjectReader {
    database_url: String,
    challenge_rate_clock_unix_seconds: Option<i64>,
}

impl ProjectCommandChallengeStore for PostgresProjectReader {
    async fn issue(
        &self,
        request: &IssueProjectCommandChallenge,
    ) -> Result<ProjectCommandChallenge, ProjectCommandChallengeError> {
        let binding = &request.binding;
        let client_session_generation = binding.client_session_generation.to_string();
        let mut client = self.connect_challenge().await?;
        let transaction = client.transaction().await.map_err(challenge_error)?;
        set_challenge_scope(&transaction, &binding.project_scope).await?;
        let inserted_idempotency = transaction
            .execute(
                "INSERT INTO storyos.command_idempotency
                   (owner_user_id, project_id, command_kind, idempotency_key, canonical_command_digest)
                 VALUES ($1::text::uuid, $2::text::uuid, $3, $4::text::uuid, $5)
                 ON CONFLICT DO NOTHING",
                &[&binding.project_scope.owner_user_id.as_ref(), &binding.project_scope.project_id.as_ref(),
                  &binding.command_kind, &binding.idempotency_key, &binding.canonical_command_digest],
            )
            .await
            .map_err(challenge_error)?
            == 1;
        let digest: String = transaction
            .query_one(
                "SELECT canonical_command_digest FROM storyos.command_idempotency
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = $3 AND idempotency_key = $4::text::uuid FOR UPDATE",
                &[
                    &binding.project_scope.owner_user_id.as_ref(),
                    &binding.project_scope.project_id.as_ref(),
                    &binding.command_kind,
                    &binding.idempotency_key,
                ],
            )
            .await
            .map_err(challenge_error)?
            .get(0);
        if digest != binding.canonical_command_digest {
            return Err(ProjectCommandChallengeError::BindingConflict);
        }
        let rate_window_bucket = if inserted_idempotency {
            Some(
                apply_challenge_rate_limit(
                    &transaction,
                    binding,
                    self.challenge_rate_clock_unix_seconds,
                )
                .await?,
            )
        } else {
            None
        };
        transaction
            .execute(
                "INSERT INTO storyos.project_command_challenges
                   (owner_user_id, project_id, command_kind, idempotency_key,
                    client_session_binding_digest, client_session_generation,
                    client_contract_revision, security_policy_revision, limit_profile_revision,
                    challenge_rate_policy_revision, challenge_rate_window_started_at,
                    method, route_template, command_schema,
                    canonical_command_digest, nonce_digest)
                 VALUES ($1::text::uuid, $2::text::uuid, $3, $4::text::uuid,
                         $5, $6::text::numeric, $7, $8, $9, $10,
                         to_timestamp($11::bigint * $12::bigint),
                         $13, $14, $15, $16, $17)
                 ON CONFLICT DO NOTHING",
                &[
                    &binding.project_scope.owner_user_id.as_ref(),
                    &binding.project_scope.project_id.as_ref(),
                    &binding.command_kind,
                    &binding.idempotency_key,
                    &binding.client_session_binding_digest,
                    &client_session_generation,
                    &binding.client_contract_revision,
                    &binding.security_policy_revision,
                    &binding.limit_profile_revision,
                    &binding.challenge_rate_policy_revision,
                    &rate_window_bucket.unwrap_or_default(),
                    &(PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS as i64),
                    &binding.method,
                    &binding.route_template,
                    &binding.command_schema,
                    &binding.canonical_command_digest,
                    &request.nonce_digest,
                ],
            )
            .await
            .map_err(challenge_error)?;
        let row = transaction
            .query_one(
                "SELECT client_session_binding_digest, client_session_generation::text,
                        client_contract_revision, security_policy_revision, limit_profile_revision,
                        challenge_rate_policy_revision,
                        method, route_template, command_schema,
                        canonical_command_digest, nonce_digest,
                        to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                 FROM storyos.project_command_challenges
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = $3 AND idempotency_key = $4::text::uuid",
                &[
                    &binding.project_scope.owner_user_id.as_ref(),
                    &binding.project_scope.project_id.as_ref(),
                    &binding.command_kind,
                    &binding.idempotency_key,
                ],
            )
            .await
            .map_err(challenge_error)?;
        let exact = row.get::<_, String>(0) == binding.client_session_binding_digest
            && row.get::<_, String>(1) == client_session_generation
            && row.get::<_, String>(2) == binding.client_contract_revision
            && row.get::<_, String>(3) == binding.security_policy_revision
            && row.get::<_, String>(4) == binding.limit_profile_revision
            && row.get::<_, String>(5) == binding.challenge_rate_policy_revision
            && row.get::<_, String>(6) == binding.method
            && row.get::<_, String>(7) == binding.route_template
            && row.get::<_, String>(8) == binding.command_schema
            && row.get::<_, String>(9) == binding.canonical_command_digest
            && row.get::<_, String>(10) == request.nonce_digest;
        if !exact {
            return Err(ProjectCommandChallengeError::BindingConflict);
        }
        let expires_at = row.get::<_, String>(11);
        transaction.commit().await.map_err(challenge_error)?;
        Ok(ProjectCommandChallenge {
            nonce: request.nonce.clone(),
            expires_at,
        })
    }
}

/// A caller-owned PostgreSQL transaction for one Project command attempt.
pub struct PostgresProjectCommandTransaction {
    client: tokio_postgres::Client,
}

impl PostgresProjectCommandTransaction {
    pub async fn commit(self) -> Result<(), ProjectCommandChallengeError> {
        self.client
            .batch_execute("COMMIT")
            .await
            .map_err(challenge_error)
    }

    pub async fn rollback(self) -> Result<(), ProjectCommandChallengeError> {
        self.client
            .batch_execute("ROLLBACK")
            .await
            .map_err(challenge_error)
    }
}

impl ProjectCommandChallengeTransaction for PostgresProjectCommandTransaction {
    async fn consume(
        &mut self,
        binding: &ProjectCommandChallengeBinding,
        nonce_digest: &str,
    ) -> Result<ProjectCommandChallengeUse, ProjectCommandChallengeError> {
        let client_session_generation = binding.client_session_generation.to_string();
        let row = self
            .client
            .query_opt(
                "SELECT challenge.client_session_binding_digest,
                        challenge.client_session_generation::text,
                        challenge.client_contract_revision, challenge.security_policy_revision,
                        challenge.limit_profile_revision, challenge.challenge_rate_policy_revision,
                        challenge.method,
                        challenge.route_template, challenge.command_schema,
                        challenge.canonical_command_digest, challenge.nonce_digest,
                        challenge.expires_at > clock_timestamp(), challenge.consumed_at IS NOT NULL,
                        idempotency.outcome_kind, idempotency.result_reference
                 FROM storyos.project_command_challenges AS challenge
                 JOIN storyos.command_idempotency AS idempotency USING
                   (owner_user_id, project_id, command_kind, idempotency_key)
                 WHERE challenge.owner_user_id = $1::text::uuid AND challenge.project_id = $2::text::uuid
                   AND challenge.command_kind = $3 AND challenge.idempotency_key = $4::text::uuid
                 FOR UPDATE OF challenge, idempotency",
                &[&binding.project_scope.owner_user_id.as_ref(), &binding.project_scope.project_id.as_ref(),
                  &binding.command_kind, &binding.idempotency_key],
            )
            .await
            .map_err(challenge_error)?
            .ok_or(ProjectCommandChallengeError::InvalidOrExpired)?;
        let exact = row.get::<_, String>(0) == binding.client_session_binding_digest
            && row.get::<_, String>(1) == client_session_generation
            && row.get::<_, String>(2) == binding.client_contract_revision
            && row.get::<_, String>(3) == binding.security_policy_revision
            && row.get::<_, String>(4) == binding.limit_profile_revision
            && row.get::<_, String>(5) == binding.challenge_rate_policy_revision
            && row.get::<_, String>(6) == binding.method
            && row.get::<_, String>(7) == binding.route_template
            && row.get::<_, String>(8) == binding.command_schema
            && row.get::<_, String>(9) == binding.canonical_command_digest
            && row.get::<_, String>(10) == nonce_digest;
        let unexpired = row.get::<_, bool>(11);
        let consumed = row.get::<_, bool>(12);
        if !exact || !consumed && !unexpired {
            return Err(ProjectCommandChallengeError::InvalidOrExpired);
        }
        let outcome = row.get::<_, String>(13);
        let result_reference = row.get::<_, Option<String>>(14);
        let use_result = if !consumed {
            self.client
                .execute(
                    "UPDATE storyos.project_command_challenges SET consumed_at = clock_timestamp()
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = $3 AND idempotency_key = $4::text::uuid",
                    &[
                        &binding.project_scope.owner_user_id.as_ref(),
                        &binding.project_scope.project_id.as_ref(),
                        &binding.command_kind,
                        &binding.idempotency_key,
                    ],
                )
                .await
                .map_err(challenge_error)?;
            self.client
                .execute(
                    "UPDATE storyos.command_idempotency SET outcome_kind = 'in_progress'
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND command_kind = $3 AND idempotency_key = $4::text::uuid",
                    &[
                        &binding.project_scope.owner_user_id.as_ref(),
                        &binding.project_scope.project_id.as_ref(),
                        &binding.command_kind,
                        &binding.idempotency_key,
                    ],
                )
                .await
                .map_err(challenge_error)?;
            ProjectCommandChallengeUse::FirstUse
        } else if outcome == "settled" {
            ProjectCommandChallengeUse::ExactRetrySettled {
                result_reference: result_reference
                    .ok_or(ProjectCommandChallengeError::InvalidOrExpired)?,
            }
        } else {
            ProjectCommandChallengeUse::ExactRetryInProgress
        };
        Ok(use_result)
    }
}

impl PostgresProjectReader {
    async fn begin_serializable_project_command_transaction(
        &self,
        scope: &ProjectScope,
    ) -> Result<PostgresProjectCommandTransaction, ProjectCommandChallengeError> {
        let client = self.connect_challenge().await?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL SERIALIZABLE")
            .await
            .map_err(challenge_error)?;
        set_challenge_scope_on_client(&client, scope).await?;
        Ok(PostgresProjectCommandTransaction { client })
    }

    pub async fn begin_project_command_transaction(
        &self,
        scope: &ProjectScope,
    ) -> Result<PostgresProjectCommandTransaction, ProjectCommandChallengeError> {
        let client = self.connect_challenge().await?;
        client
            .batch_execute("BEGIN")
            .await
            .map_err(challenge_error)?;
        set_challenge_scope_on_client(&client, scope).await?;
        Ok(PostgresProjectCommandTransaction { client })
    }

    async fn connect_challenge(
        &self,
    ) -> Result<tokio_postgres::Client, ProjectCommandChallengeError> {
        let (client, connection) = tokio_postgres::connect(&self.database_url, NoTls)
            .await
            .map_err(challenge_error)?;
        tokio::spawn(async move {
            let _connection_result = connection.await;
        });
        Ok(client)
    }
}

async fn apply_challenge_rate_limit(
    transaction: &tokio_postgres::Transaction<'_>,
    binding: &ProjectCommandChallengeBinding,
    clock_unix_seconds: Option<i64>,
) -> Result<i64, ProjectCommandChallengeError> {
    let client_session_generation = binding.client_session_generation.to_string();
    transaction
        .execute(
            "INSERT INTO storyos.project_command_challenge_rate_guards
               (owner_user_id, project_id, client_session_generation, policy_revision)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4)
             ON CONFLICT DO NOTHING",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &client_session_generation,
                &PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION,
            ],
        )
        .await
        .map_err(challenge_error)?;
    transaction
        .query_one(
            "SELECT policy_revision
             FROM storyos.project_command_challenge_rate_guards
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND client_session_generation = $3::text::numeric AND policy_revision = $4
             FOR UPDATE",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &client_session_generation,
                &PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION,
            ],
        )
        .await
        .map_err(challenge_error)?;
    let selected_clock = transaction
        .query_one(
            "SELECT selected.unix_seconds,
                    selected.unix_seconds / $1::bigint AS minute_bucket
             FROM (
               SELECT coalesce(
                 $2::bigint,
                 floor(extract(epoch FROM clock_timestamp()))::bigint
               ) AS unix_seconds
             ) AS selected",
            &[
                &(PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS as i64),
                &clock_unix_seconds,
            ],
        )
        .await
        .map_err(challenge_error)?;
    let selected_unix_seconds = selected_clock.get::<_, i64>(0);
    let minute_bucket = selected_clock.get::<_, i64>(1);
    transaction
        .execute(
            "INSERT INTO storyos.project_command_challenge_rate_windows
               (owner_user_id, project_id, client_session_generation, policy_revision,
                window_started_at, issued_count)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4,
                     to_timestamp($5::bigint * $6::bigint), 0)
             ON CONFLICT DO NOTHING",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &client_session_generation,
                &PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION,
                &minute_bucket,
                &(PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS as i64),
            ],
        )
        .await
        .map_err(challenge_error)?;
    let row = transaction
        .query_one(
            "SELECT issued_count
             FROM storyos.project_command_challenge_rate_windows
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND client_session_generation = $3::text::numeric AND policy_revision = $4
               AND window_started_at = to_timestamp($5::bigint * $6::bigint)
             FOR UPDATE",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &client_session_generation,
                &PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION,
                &minute_bucket,
                &(PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS as i64),
            ],
        )
        .await
        .map_err(challenge_error)?;
    let issued_count = row.get::<_, i16>(0);
    if issued_count >= PROJECT_COMMAND_CHALLENGE_RATE_CAPACITY {
        let window_seconds = PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS as i64;
        let retry_after_seconds = ((minute_bucket + 1) * window_seconds - selected_unix_seconds)
            .clamp(1, window_seconds) as u64;
        return Err(ProjectCommandChallengeError::RateLimited {
            retry_after_seconds,
        });
    }
    transaction
        .execute(
            "UPDATE storyos.project_command_challenge_rate_windows SET issued_count = issued_count + 1
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND client_session_generation = $3::text::numeric AND policy_revision = $4
               AND window_started_at = to_timestamp($5::bigint * $6::bigint)",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &client_session_generation,
                &PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION,
                &minute_bucket,
                &(PROJECT_COMMAND_CHALLENGE_RATE_WINDOW_SECONDS as i64),
            ],
        )
        .await
        .map_err(challenge_error)?;
    Ok(minute_bucket)
}

async fn set_challenge_scope_on_client(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<(), ProjectCommandChallengeError> {
    client
        .execute(
            "SELECT set_config('storyos.user_id', $1, true),
                set_config('storyos.owner_user_id', $1, true),
                set_config('storyos.project_id', $2, true)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map(|_| ())
        .map_err(challenge_error)
}

async fn set_challenge_scope(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
) -> Result<(), ProjectCommandChallengeError> {
    transaction
        .execute(
            "SELECT set_config('storyos.user_id', $1, true),
                set_config('storyos.owner_user_id', $1, true),
                set_config('storyos.project_id', $2, true)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map(|_| ())
        .map_err(challenge_error)
}

fn challenge_error(source: tokio_postgres::Error) -> ProjectCommandChallengeError {
    ProjectCommandChallengeError::Unavailable(Box::new(source))
}

impl PostgresProjectReader {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            challenge_rate_clock_unix_seconds: None,
        }
    }

    async fn connect(&self) -> Result<tokio_postgres::Client, ProjectReadError> {
        let (client, connection) = tokio_postgres::connect(&self.database_url, NoTls)
            .await
            .map_err(read_error)?;
        tokio::spawn(async move {
            let _connection_result = connection.await;
        });
        Ok(client)
    }
}

impl ProjectReader for PostgresProjectReader {
    async fn read_project(
        &self,
        scope: &ProjectScope,
    ) -> Result<Option<Project>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let project = transaction
            .query_opt(
                "SELECT project_id::text, title, current_chapter_id::text \
                 FROM storyos.projects \
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
            .map(|row| Project {
                project_id: ProjectId::new(row.get::<_, String>(0)),
                title: row.get(1),
                current_chapter_id: ChapterId::new(row.get::<_, String>(2)),
            });
        transaction.commit().await.map_err(read_error)?;
        Ok(project)
    }

    async fn read_chapter(
        &self,
        scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> Result<Option<Chapter>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let chapter = transaction
            .query_opt(
                "SELECT object.manuscript_object_id::text, object.title, \
                        revision.revision_id::text, convert_from(payload.canonical_bytes, 'UTF8'), \
                        COALESCE(counters.project_activity_position, 0)::text \
                 FROM storyos.manuscript_objects AS object \
                 JOIN storyos.projects AS project \
                   ON (project.owner_user_id, project.project_id, project.current_chapter_id) = \
                      (object.owner_user_id, object.project_id, object.manuscript_object_id) \
                 JOIN storyos.authoritative_heads AS head \
                   ON (head.owner_user_id, head.project_id, head.manuscript_object_id) = \
                      (object.owner_user_id, object.project_id, object.manuscript_object_id) \
                 JOIN storyos.authoritative_revisions AS revision \
                   ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id) = \
                      (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id) \
                 JOIN storyos.authoritative_payloads AS payload \
                   ON (payload.owner_user_id, payload.project_id, payload.payload_id) = \
                      (revision.owner_user_id, revision.project_id, revision.payload_id) \
                 LEFT JOIN storyos.scope_counters AS counters \
                   ON (counters.owner_user_id, counters.project_id) = \
                      (object.owner_user_id, object.project_id) \
                 WHERE object.owner_user_id = $1::text::uuid AND object.project_id = $2::text::uuid \
                   AND object.manuscript_object_id = $3::text::uuid AND object.object_kind = 'chapter'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &chapter_id.as_ref(),
                ],
            )
            .await
            .map_err(read_error)?
            .map(|row| -> Result<Chapter, ProjectReadError> { Ok(Chapter {
                chapter_id: ChapterId::new(row.get::<_, String>(0)),
                title: row.get(1),
                revision_id: RevisionId::new(row.get::<_, String>(2)),
                body: row.get(3),
                project_activity_position: row
                    .get::<_, String>(4)
                    .parse()
                    .map_err(ProjectReadError::unavailable)?,
            })})
            .transpose()?;
        transaction.commit().await.map_err(read_error)?;
        Ok(chapter)
    }
}

async fn set_scope(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
) -> Result<(), ProjectReadError> {
    transaction
        .execute(
            "SELECT set_config('storyos.user_id', $1, true), \
             set_config('storyos.owner_user_id', $1, true), \
             set_config('storyos.project_id', $2, true)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map(|_| ())
        .map_err(read_error)
}

fn read_error(source: tokio_postgres::Error) -> ProjectReadError {
    ProjectReadError::unavailable(source)
}
