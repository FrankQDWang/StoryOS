use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptanceRefusal, AcceptanceRefusalBoundary,
    AcceptanceRefusalReason, ProjectReadError, ProjectScope,
};
use uuid::Uuid;

use crate::accept_proposal::{accept_database_error, accept_parse_error};
use crate::{PostgresProjectReader, read_error, set_scope};

pub(super) fn parse_reason(value: &str) -> Result<AcceptanceRefusalReason, std::io::Error> {
    match value {
        "stale_writer" => Ok(AcceptanceRefusalReason::StaleWriter),
        "session_changed" => Ok(AcceptanceRefusalReason::SessionChanged),
        "invalid_challenge" => Ok(AcceptanceRefusalReason::InvalidChallenge),
        _ => Err(std::io::Error::other("unknown Acceptance refusal reason")),
    }
}

impl PostgresProjectReader {
    pub(super) async fn retain_acceptance_refusal(
        &self,
        command: &AcceptProposalCommand,
        reason: AcceptanceRefusalReason,
        boundary: AcceptanceRefusalBoundary,
    ) -> Result<AcceptanceRefusalReason, AcceptProposalError> {
        let mut client = self.connect().await.map_err(accept_parse_error)?;
        let transaction = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::Serializable)
            .start()
            .await
            .map_err(accept_database_error)?;
        set_scope(&transaction, &command.project_scope)
            .await
            .map_err(accept_parse_error)?;
        let binding = &command.challenge_binding;
        let generation = binding.client_session_generation.to_string();
        let proved = transaction.query_opt(
            "SELECT challenge.client_contract_revision, challenge.security_policy_revision,
                    challenge.limit_profile_revision, challenge.challenge_rate_policy_revision
             FROM storyos.project_command_challenges AS challenge
             JOIN storyos.command_idempotency AS idempotency USING
               (owner_user_id, project_id, command_kind, idempotency_key)
             JOIN storyos.proposals AS proposal USING (owner_user_id, project_id)
             WHERE challenge.owner_user_id = $1::text::uuid AND challenge.project_id = $2::text::uuid
               AND proposal.proposal_id = $3::text::uuid
               AND challenge.command_kind = 'acceptProposal' AND challenge.idempotency_key = $4::text::uuid
               AND challenge.canonical_command_digest = $5 AND challenge.client_session_binding_digest = $6
               AND challenge.client_session_generation = $7::text::numeric
               AND challenge.client_contract_revision = $8 AND challenge.security_policy_revision = $9
               AND challenge.limit_profile_revision = $10 AND challenge.challenge_rate_policy_revision = $11
               AND challenge.method = $12 AND challenge.route_template = $13 AND challenge.command_schema = $14
               AND challenge.consumed_at IS NULL AND idempotency.outcome_kind = 'pending'
               AND NOT EXISTS (SELECT 1 FROM storyos.author_command_admissions AS admission
                 WHERE (admission.owner_user_id, admission.project_id, admission.command_kind, admission.idempotency_key) =
                       (challenge.owner_user_id, challenge.project_id, challenge.command_kind, challenge.idempotency_key))
             FOR UPDATE OF challenge, idempotency",
            &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(),
              &command.proposal_id, &binding.idempotency_key, &binding.canonical_command_digest,
              &binding.client_session_binding_digest, &generation, &binding.client_contract_revision,
              &binding.security_policy_revision, &binding.limit_profile_revision, &binding.challenge_rate_policy_revision,
              &binding.method, &binding.route_template, &binding.command_schema],
        ).await.map_err(accept_database_error)?;
        let Some(proved) = proved else {
            return Err(AcceptProposalError::InvalidChallenge);
        };
        let reason = match reason {
            AcceptanceRefusalReason::StaleWriter => "stale_writer",
            AcceptanceRefusalReason::SessionChanged => "session_changed",
            AcceptanceRefusalReason::InvalidChallenge => "invalid_challenge",
        };
        let boundary = match boundary {
            AcceptanceRefusalBoundary::Challenge => "challenge",
            AcceptanceRefusalBoundary::WriterSession => "writer_session",
        };
        transaction.execute(
            "INSERT INTO storyos.acceptance_refusals
              (owner_user_id, project_id, proposal_id, refusal_id, idempotency_key, correlation_id,
               reason, client_contract_revision, security_policy_revision, limit_profile_revision, challenge_rate_policy_revision, boundary)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid,
                     $6::text::uuid, $7, $8, $9, $10, $11, $12) ON CONFLICT DO NOTHING",
            &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(),
              &command.proposal_id, &Uuid::now_v7().to_string(), &binding.idempotency_key, &command.correlation_id,
              &reason, &proved.get::<_, String>(0), &proved.get::<_, String>(1),
              &proved.get::<_, String>(2), &proved.get::<_, String>(3), &boundary],
        ).await.map_err(accept_database_error)?;
        let row = transaction.query_one(
            "SELECT reason FROM storyos.acceptance_refusals
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid AND idempotency_key = $3::text::uuid",
            &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(), &binding.idempotency_key],
        ).await.map_err(accept_database_error)?;
        let reason = parse_reason(row.get::<_, &str>(0)).map_err(accept_parse_error)?;
        transaction.commit().await.map_err(accept_database_error)?;
        Ok(reason)
    }
}

pub(super) async fn read_latest_refusal(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
    proposal_id: &str,
) -> Result<Option<AcceptanceRefusal>, ProjectReadError> {
    let row = transaction.query_opt(
        "SELECT refusal_id::text, correlation_id::text, reason, client_contract_revision,
                security_policy_revision, limit_profile_revision, challenge_rate_policy_revision,
                to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'), boundary = 'challenge', command_schema, refusal_profile_revision
         FROM storyos.acceptance_refusals WHERE owner_user_id = $1::text::uuid
           AND project_id = $2::text::uuid AND proposal_id = $3::text::uuid
         ORDER BY refusal_id DESC LIMIT 1",
        &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref(), &proposal_id],
    ).await.map_err(read_error)?;
    row.map(|row| {
        Ok(AcceptanceRefusal {
            refusal_id: row.get(0),
            correlation_id: row.get(1),
            reason: parse_reason(row.get::<_, &str>(2)).map_err(ProjectReadError::unavailable)?,
            boundary: if row.get::<_, bool>(8) {
                AcceptanceRefusalBoundary::Challenge
            } else {
                AcceptanceRefusalBoundary::WriterSession
            },
            command_schema: row.get(9),
            refusal_profile_revision: row.get(10),
            client_contract_revision: row.get(3),
            security_policy_revision: row.get(4),
            limit_profile_revision: row.get(5),
            challenge_rate_policy_revision: row.get(6),
            recorded_at: row.get(7),
        })
    })
    .transpose()
}
