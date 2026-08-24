use storyos_application::{
    CreateProjectChallenge, CreateProjectChallengeStore, IssueCreateProjectChallenge,
    ProjectCommandChallengeError, ProjectId,
};

use super::{PostgresProjectReader, challenge_error};

impl CreateProjectChallengeStore for PostgresProjectReader {
    async fn issue(
        &self,
        request: &IssueCreateProjectChallenge,
    ) -> Result<CreateProjectChallenge, ProjectCommandChallengeError> {
        let binding = &request.binding;
        let client_session_generation = binding.client_session_generation.to_string();
        let mut client = self.connect_challenge().await?;
        let transaction = client.transaction().await.map_err(challenge_error)?;
        transaction
            .execute(
                "SELECT set_config('storyos.user_id', $1, true)",
                &[&binding.owner_user_id.as_ref()],
            )
            .await
            .map_err(challenge_error)?;
        let user = transaction
            .query_opt(
                "SELECT user_id::text FROM storyos.users WHERE user_id = $1::text::uuid",
                &[&binding.owner_user_id.as_ref()],
            )
            .await
            .map_err(challenge_error)?;
        if user.is_none() {
            return Err(ProjectCommandChallengeError::Unavailable(Box::new(
                std::io::Error::other("trusted User is absent"),
            )));
        }
        transaction
            .execute(
                "INSERT INTO storyos.create_project_idempotency
                   (user_id, idempotency_key, prospective_project_id, create_input_digest,
                    canonical_command_digest)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5)
                 ON CONFLICT (user_id, idempotency_key) DO NOTHING",
                &[
                    &binding.owner_user_id.as_ref(),
                    &binding.idempotency_key,
                    &binding.prospective_project_id.as_ref(),
                    &binding.create_input_digest,
                    &binding.canonical_command_digest,
                ],
            )
            .await
            .map_err(challenge_error)?;
        let idempotency = transaction
            .query_one(
                "SELECT prospective_project_id::text, create_input_digest, canonical_command_digest
                 FROM storyos.create_project_idempotency
                 WHERE user_id = $1::text::uuid AND idempotency_key = $2::text::uuid
                 FOR UPDATE",
                &[&binding.owner_user_id.as_ref(), &binding.idempotency_key],
            )
            .await
            .map_err(challenge_error)?;
        if idempotency.get::<_, String>(1) != binding.create_input_digest {
            return Err(ProjectCommandChallengeError::BindingConflict);
        }
        let prospective_project_id: String = idempotency.get(0);
        let canonical_command_digest: String = idempotency.get(2);
        transaction
            .execute(
                "INSERT INTO storyos.create_project_challenges
                   (user_id, idempotency_key, prospective_project_id,
                    client_session_binding_digest, client_session_generation,
                    client_contract_revision, security_policy_revision, limit_profile_revision,
                    method, route_template, command_schema, command_kind,
                    canonical_command_digest, nonce_digest)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                         $4, $5::text::numeric, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                 ON CONFLICT (user_id, idempotency_key) DO NOTHING",
                &[
                    &binding.owner_user_id.as_ref(),
                    &binding.idempotency_key,
                    &prospective_project_id,
                    &binding.client_session_binding_digest,
                    &client_session_generation,
                    &binding.client_contract_revision,
                    &binding.security_policy_revision,
                    &binding.limit_profile_revision,
                    &binding.method,
                    &binding.route_template,
                    &binding.command_schema,
                    &binding.command_kind,
                    &canonical_command_digest,
                    &request.nonce_digest,
                ],
            )
            .await
            .map_err(challenge_error)?;
        let row = transaction
            .query_one(
                "SELECT client_session_binding_digest, client_session_generation::text,
                        client_contract_revision, security_policy_revision, limit_profile_revision,
                        method, route_template, command_schema, command_kind,
                        to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                 FROM storyos.create_project_challenges
                 WHERE user_id = $1::text::uuid AND idempotency_key = $2::text::uuid",
                &[&binding.owner_user_id.as_ref(), &binding.idempotency_key],
            )
            .await
            .map_err(challenge_error)?;
        let exact = row.get::<_, String>(0) == binding.client_session_binding_digest
            && row.get::<_, String>(1) == client_session_generation
            && row.get::<_, String>(2) == binding.client_contract_revision
            && row.get::<_, String>(3) == binding.security_policy_revision
            && row.get::<_, String>(4) == binding.limit_profile_revision
            && row.get::<_, String>(5) == binding.method
            && row.get::<_, String>(6) == binding.route_template
            && row.get::<_, String>(7) == binding.command_schema
            && row.get::<_, String>(8) == binding.command_kind;
        if !exact {
            return Err(ProjectCommandChallengeError::BindingConflict);
        }
        let expires_at = row.get::<_, String>(9);
        transaction.commit().await.map_err(challenge_error)?;
        Ok(CreateProjectChallenge {
            prospective_project_id: ProjectId::new(prospective_project_id),
            canonical_command_digest,
            expires_at,
        })
    }
}
