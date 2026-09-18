SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.acceptance_refusals (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  refusal_id uuid NOT NULL,
  idempotency_key uuid NOT NULL,
  correlation_id uuid NOT NULL,
  command_kind text NOT NULL DEFAULT 'acceptProposal' CHECK (command_kind = 'acceptProposal'),
  boundary text NOT NULL CHECK (boundary IN ('challenge', 'writer_session')),
  command_schema text NOT NULL DEFAULT 'storyos.command.accept-proposal.request.v1'
    CHECK (command_schema = 'storyos.command.accept-proposal.request.v1'),
  refusal_profile_revision text NOT NULL DEFAULT 'storyos.acceptance-refusal.fixed-fields.v1'
    CHECK (refusal_profile_revision = 'storyos.acceptance-refusal.fixed-fields.v1'),
  reason text NOT NULL CHECK (reason IN ('stale_writer', 'session_changed', 'invalid_challenge')),
  client_contract_revision text NOT NULL CHECK (client_contract_revision = 'storyos.web-client.release-1.v3'),
  security_policy_revision text NOT NULL CHECK (security_policy_revision = 'storyos.web-security-policy.release-1.v1'),
  limit_profile_revision text NOT NULL CHECK (limit_profile_revision = 'storyos.foundation.absolute.v1'),
  challenge_rate_policy_revision text NOT NULL CHECK (challenge_rate_policy_revision = 'storyos.project-command-challenge-rate.fixed-window.v1'),
  recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, idempotency_key),
  UNIQUE (owner_user_id, project_id, refusal_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id)
    REFERENCES storyos.proposals (owner_user_id, project_id, proposal_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key)
    REFERENCES storyos.project_command_challenges
      (owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL
);
CREATE INDEX acceptance_refusals_proposal_idx ON storyos.acceptance_refusals
  (owner_user_id, project_id, proposal_id, refusal_id DESC);
ALTER TABLE storyos.acceptance_refusals ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.acceptance_refusals FORCE ROW LEVEL SECURITY;
CREATE POLICY acceptance_refusals_exact_scope ON storyos.acceptance_refusals USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.acceptance_refusals TO storyos_runtime;
