CREATE TABLE storyos.command_idempotency (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  command_kind text NOT NULL,
  idempotency_key uuid NOT NULL,
  canonical_command_digest text NOT NULL,
  outcome_kind text NOT NULL DEFAULT 'pending'
    CHECK (outcome_kind IN ('pending', 'in_progress', 'settled')),
  result_reference text,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, command_kind, idempotency_key),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects (owner_user_id, project_id)
);

CREATE TABLE storyos.project_command_challenge_rate_guards (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  client_session_generation bigint NOT NULL CHECK (client_session_generation >= 0),
  policy_revision text NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, client_session_generation, policy_revision),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects (owner_user_id, project_id)
);

CREATE TABLE storyos.project_command_challenge_rate_windows (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  client_session_generation bigint NOT NULL CHECK (client_session_generation >= 0),
  policy_revision text NOT NULL,
  window_started_at timestamptz NOT NULL,
  issued_count smallint NOT NULL DEFAULT 0 CHECK (issued_count BETWEEN 0 AND 10),
  PRIMARY KEY (
    owner_user_id, project_id, client_session_generation,
    policy_revision, window_started_at
  ),
  FOREIGN KEY (owner_user_id, project_id, client_session_generation, policy_revision)
    REFERENCES storyos.project_command_challenge_rate_guards
      (owner_user_id, project_id, client_session_generation, policy_revision)
);

CREATE TABLE storyos.project_command_challenges (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  command_kind text NOT NULL,
  idempotency_key uuid NOT NULL,
  client_session_binding_digest text NOT NULL,
  client_session_generation bigint NOT NULL CHECK (client_session_generation >= 0),
  client_contract_revision text NOT NULL,
  security_policy_revision text NOT NULL,
  limit_profile_revision text NOT NULL,
  challenge_rate_policy_revision text NOT NULL,
  challenge_rate_window_started_at timestamptz NOT NULL,
  method text NOT NULL,
  route_template text NOT NULL,
  command_schema text NOT NULL,
  canonical_command_digest text NOT NULL,
  nonce_digest text NOT NULL UNIQUE,
  issued_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  expires_at timestamptz NOT NULL DEFAULT clock_timestamp() + interval '5 minutes',
  consumed_at timestamptz,
  PRIMARY KEY (owner_user_id, project_id, command_kind, idempotency_key),
  FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key)
    REFERENCES storyos.command_idempotency
      (owner_user_id, project_id, command_kind, idempotency_key),
  FOREIGN KEY (
    owner_user_id, project_id, client_session_generation,
    challenge_rate_policy_revision, challenge_rate_window_started_at
  ) REFERENCES storyos.project_command_challenge_rate_windows (
    owner_user_id, project_id, client_session_generation,
    policy_revision, window_started_at
  )
);

ALTER TABLE storyos.command_idempotency ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.command_idempotency FORCE ROW LEVEL SECURITY;
CREATE POLICY command_idempotency_exact_scope ON storyos.command_idempotency
  USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  )
  WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

ALTER TABLE storyos.project_command_challenges ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.project_command_challenges FORCE ROW LEVEL SECURITY;
CREATE POLICY project_command_challenges_exact_scope ON storyos.project_command_challenges
  USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  )
  WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

ALTER TABLE storyos.project_command_challenge_rate_windows ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.project_command_challenge_rate_windows FORCE ROW LEVEL SECURITY;
CREATE POLICY project_command_challenge_rate_windows_exact_scope
  ON storyos.project_command_challenge_rate_windows
  USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  )
  WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

ALTER TABLE storyos.project_command_challenge_rate_guards ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.project_command_challenge_rate_guards FORCE ROW LEVEL SECURITY;
CREATE POLICY project_command_challenge_rate_guards_exact_scope
  ON storyos.project_command_challenge_rate_guards
  USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  )
  WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT, UPDATE ON storyos.command_idempotency,
  storyos.project_command_challenges,
  storyos.project_command_challenge_rate_guards,
  storyos.project_command_challenge_rate_windows TO storyos_runtime;
