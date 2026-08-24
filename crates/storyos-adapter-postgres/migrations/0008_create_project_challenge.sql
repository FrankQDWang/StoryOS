SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.create_project_idempotency (
  user_id uuid NOT NULL REFERENCES storyos.users (user_id),
  idempotency_key uuid NOT NULL,
  prospective_project_id uuid NOT NULL UNIQUE,
  create_input_digest text NOT NULL,
  canonical_command_digest text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (user_id, idempotency_key)
);

CREATE TABLE storyos.create_project_challenges (
  user_id uuid NOT NULL,
  idempotency_key uuid NOT NULL,
  prospective_project_id uuid NOT NULL,
  client_session_binding_digest text NOT NULL,
  client_session_generation numeric(20, 0) NOT NULL
    CHECK (client_session_generation BETWEEN 0 AND 18446744073709551615),
  client_contract_revision text NOT NULL,
  security_policy_revision text NOT NULL,
  limit_profile_revision text NOT NULL,
  method text NOT NULL,
  route_template text NOT NULL,
  command_schema text NOT NULL,
  command_kind text NOT NULL CHECK (command_kind = 'createProject'),
  canonical_command_digest text NOT NULL,
  nonce_digest text NOT NULL UNIQUE,
  issued_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  expires_at timestamptz NOT NULL DEFAULT clock_timestamp() + interval '5 minutes',
  PRIMARY KEY (user_id, idempotency_key),
  FOREIGN KEY (user_id, idempotency_key)
    REFERENCES storyos.create_project_idempotency (user_id, idempotency_key)
);

ALTER TABLE storyos.create_project_idempotency ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.create_project_idempotency FORCE ROW LEVEL SECURITY;
CREATE POLICY create_project_idempotency_exact_user ON storyos.create_project_idempotency
  USING (user_id = NULLIF(current_setting('storyos.user_id', true), '')::uuid)
  WITH CHECK (user_id = NULLIF(current_setting('storyos.user_id', true), '')::uuid);

ALTER TABLE storyos.create_project_challenges ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.create_project_challenges FORCE ROW LEVEL SECURITY;
CREATE POLICY create_project_challenges_exact_user ON storyos.create_project_challenges
  USING (user_id = NULLIF(current_setting('storyos.user_id', true), '')::uuid)
  WITH CHECK (user_id = NULLIF(current_setting('storyos.user_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE ON storyos.create_project_idempotency,
  storyos.create_project_challenges TO storyos_runtime;
